//! OCI-compliant registry server implementation.

use crate::config::RegistryConfig;
use crate::error::Result;
use crate::storage::{create_storage, ManifestEntry, Storage};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, head, patch, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, warn};

fn strip_leading_slash(s: &str) -> &str {
    s.strip_prefix('/').unwrap_or(s)
}

type SharedStorage = Arc<dyn Storage>;

#[derive(Clone)]
struct AppState {
    storage: SharedStorage,
}

#[derive(Serialize)]
struct ApiVersion {
    version: String,
}

#[derive(Deserialize)]
struct UploadParams {
    digest: Option<String>,
}

/// The main registry server.
///
/// Implements an OCI-compliant container registry that can be used for
/// testing Docker/container workflows.
pub struct RegistryServer {
    addr: SocketAddr,
    _handle: tokio::task::JoinHandle<()>,
}

impl RegistryServer {
    /// Creates and starts a new registry server with the given configuration.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use registry_testkit::{RegistryServer, RegistryConfig};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = RegistryConfig::memory();
    /// let server = RegistryServer::new(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(config: RegistryConfig) -> Result<Self> {
        let storage = create_storage(&config.storage).await?;

        let state = AppState { storage };

        let app = Router::new()
            .route("/v2/", get(api_version))
            .route("/v2/{name}/blobs/{digest}", head(check_blob))
            .route("/v2/{name}/blobs/{digest}", get(get_blob))
            .route("/v2/{name}/blobs/uploads/", post(start_upload))
            .route("/v2/{name}/blobs/uploads/{uuid}", patch(upload_chunk))
            .route("/v2/{name}/blobs/uploads/{uuid}", put(finish_upload))
            .route("/v2/{name}/manifests/{reference}", put(put_manifest))
            .route("/v2/{name}/manifests/{reference}", get(get_manifest))
            .route("/v2/{name}/manifests/{reference}", head(check_manifest))
            .layer(
                tower::ServiceBuilder::new()
                    .layer(axum::extract::DefaultBodyLimit::max(512 * 1024 * 1024))
                    .layer(TraceLayer::new_for_http()),
            )
            .with_state(state);

        let bind_addr = if let Some(port) = config.port {
            format!("{}:{}", config.host, port)
        } else {
            format!("{}:0", config.host)
        };

        let listener = TcpListener::bind(&bind_addr).await?;
        let addr = listener.local_addr()?;

        info!("Registry listening on {}", addr);

        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        Ok(Self {
            addr,
            _handle: handle,
        })
    }

    /// Returns the socket address the server is bound to.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Returns the full URL of the registry server.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use registry_testkit::{RegistryServer, RegistryConfig};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let server = RegistryServer::new(RegistryConfig::memory()).await?;
    /// println!("Registry URL: {}", server.url());
    /// # Ok(())
    /// # }
    /// ```
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }

    /// Returns the port number the server is listening on.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }
}

async fn api_version() -> Json<ApiVersion> {
    Json(ApiVersion {
        version: "registry/2.0".to_string(),
    })
}

async fn check_blob(
    State(state): State<AppState>,
    Path((name, digest)): Path<(String, String)>,
) -> impl IntoResponse {
    let name = strip_leading_slash(&name);
    info!("Checking blob: {}/{}", name, digest);

    match state.storage.get_blob(&digest).await {
        Ok(Some(blob)) => (StatusCode::OK, [("Content-Length", blob.len().to_string())]),
        _ => (StatusCode::NOT_FOUND, [("Content-Length", "0".to_string())]),
    }
}

async fn get_blob(
    State(state): State<AppState>,
    Path((name, digest)): Path<(String, String)>,
) -> impl IntoResponse {
    let name = strip_leading_slash(&name);
    info!("Getting blob: {}/{}", name, digest);

    match state.storage.get_blob(&digest).await {
        Ok(Some(blob)) => (StatusCode::OK, blob),
        _ => (StatusCode::NOT_FOUND, vec![]),
    }
}

async fn start_upload(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let name = strip_leading_slash(&name);
    let uuid = uuid::Uuid::new_v4().to_string();
    info!("Starting upload: {} ({})", name, uuid);

    if let Err(e) = state.storage.create_upload(uuid.clone()).await {
        warn!("Failed to create upload: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("Location", String::new())],
        );
    }

    (
        StatusCode::ACCEPTED,
        [("Location", format!("/v2/{}/blobs/uploads/{}", name, uuid))],
    )
}

async fn upload_chunk(
    State(state): State<AppState>,
    Path((name, uuid)): Path<(String, String)>,
    body: Bytes,
) -> impl IntoResponse {
    let name = strip_leading_slash(&name);
    debug!("Uploading chunk: {}/{} ({} bytes)", name, uuid, body.len());

    match state.storage.append_upload(&uuid, &body).await {
        Ok(_) => {
            let end = body.len().saturating_sub(1);
            (
                StatusCode::ACCEPTED,
                [
                    ("Location", format!("/v2/{}/blobs/uploads/{}", name, uuid)),
                    ("Range", format!("0-{}", end)),
                    ("Docker-Upload-UUID", uuid),
                ],
            )
        }
        Err(_) => {
            warn!("Upload not found: {}", uuid);
            (
                StatusCode::NOT_FOUND,
                [
                    ("Location", String::new()),
                    ("Range", String::new()),
                    ("Docker-Upload-UUID", String::new()),
                ],
            )
        }
    }
}

async fn finish_upload(
    State(state): State<AppState>,
    Path((name, uuid)): Path<(String, String)>,
    Query(params): Query<UploadParams>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let name = strip_leading_slash(&name);
    debug!("Finishing upload: {}/{}", name, uuid);

    let upload_data = match state.storage.finish_upload(&uuid).await {
        Ok(Some(mut data)) => {
            data.extend_from_slice(&body);
            data
        }
        _ => {
            warn!("Upload not found: {}", uuid);
            return (
                StatusCode::NOT_FOUND,
                [
                    ("Location", String::new()),
                    ("Docker-Content-Digest", String::new()),
                ],
            );
        }
    };

    let digest_str = params
        .digest
        .or_else(|| {
            headers
                .get("digest")
                .or_else(|| headers.get("Docker-Content-Digest"))
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| {
            let mut hasher = Sha256::new();
            hasher.update(&upload_data);
            format!("sha256:{}", hex::encode(hasher.finalize()))
        });

    if let Err(e) = state
        .storage
        .store_blob(digest_str.clone(), upload_data)
        .await
    {
        warn!("Failed to store blob: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [
                ("Location", String::new()),
                ("Docker-Content-Digest", String::new()),
            ],
        );
    }

    info!("Stored blob: {}", digest_str);

    (
        StatusCode::CREATED,
        [
            ("Location", format!("/v2/{}/blobs/{}", name, digest_str)),
            ("Docker-Content-Digest", digest_str),
        ],
    )
}

async fn put_manifest(
    State(state): State<AppState>,
    Path((name, reference)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let name = strip_leading_slash(&name);
    info!("Putting manifest: {}/{}", name, reference);

    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/vnd.docker.distribution.manifest.v2+json")
        .to_string();

    let mut hasher = Sha256::new();
    hasher.update(&body);
    let digest = format!("sha256:{}", hex::encode(hasher.finalize()));

    let entry = ManifestEntry {
        data: body.to_vec(),
        content_type: content_type.clone(),
    };

    let key = format!("{}:{}", name, reference);
    let digest_key = format!("{}:{}", name, digest);

    if let Err(e) = state.storage.store_manifest(key, entry.clone()).await {
        warn!("Failed to store manifest: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [
                ("Location", String::new()),
                ("Content-Type", String::new()),
                ("Docker-Content-Digest", String::new()),
            ],
        );
    }

    if let Err(e) = state.storage.store_manifest(digest_key, entry).await {
        warn!("Failed to store manifest by digest: {}", e);
    }

    info!(
        "Stored manifest with digest: {} (type: {})",
        digest, content_type
    );

    (
        StatusCode::CREATED,
        [
            ("Location", format!("/v2/{}/manifests/{}", name, reference)),
            ("Content-Type", content_type),
            ("Docker-Content-Digest", digest),
        ],
    )
}

async fn get_manifest(
    State(state): State<AppState>,
    Path((name, reference)): Path<(String, String)>,
) -> impl IntoResponse {
    let name = strip_leading_slash(&name);
    info!("Getting manifest: {}/{}", name, reference);

    let key = format!("{}:{}", name, reference);

    match state.storage.get_manifest(&key).await {
        Ok(Some(entry)) => (
            StatusCode::OK,
            [("Content-Type", entry.content_type)],
            entry.data,
        ),
        _ => (
            StatusCode::NOT_FOUND,
            [("Content-Type", "text/plain".to_string())],
            vec![],
        ),
    }
}

async fn check_manifest(
    State(state): State<AppState>,
    Path((name, reference)): Path<(String, String)>,
) -> impl IntoResponse {
    let name = strip_leading_slash(&name);
    info!("Checking manifest: {}/{}", name, reference);

    let key = format!("{}:{}", name, reference);

    match state.storage.get_manifest(&key).await {
        Ok(Some(entry)) => {
            let mut hasher = Sha256::new();
            hasher.update(&entry.data);
            let digest = format!("sha256:{}", hex::encode(hasher.finalize()));

            (
                StatusCode::OK,
                [
                    ("Content-Type", entry.content_type),
                    ("Docker-Content-Digest", digest),
                ],
            )
        }
        _ => (
            StatusCode::NOT_FOUND,
            [
                ("Content-Type", "text/plain".to_string()),
                ("Docker-Content-Digest", String::new()),
            ],
        ),
    }
}
