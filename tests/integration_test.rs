use bollard::query_parameters::{CreateImageOptions, ListImagesOptions, PushImageOptions, RemoveImageOptions, TagImageOptions};
use bollard::Docker;
use futures_util::stream::StreamExt;
use registry_testkit::{RegistryConfig, RegistryServer};

#[tokio::test]
async fn test_memory_storage() {
    let config = RegistryConfig::memory().with_port(0);
    let server = RegistryServer::new(config).await.unwrap();

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/v2/", server.url()))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let json: serde_json::Value = response.json().await.unwrap();
    assert_eq!(json["version"], "registry/2.0");
}

#[tokio::test]
async fn test_temp_dir_storage() {
    let config = RegistryConfig::temp_dir().with_port(0);
    let server = RegistryServer::new(config).await.unwrap();

    let client = reqwest::Client::new();
    let response = client
        .get(format!("{}/v2/", server.url()))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_blob_operations() {
    let config = RegistryConfig::memory().with_port(0);
    let server = RegistryServer::new(config).await.unwrap();

    let client = reqwest::Client::new();

    let blob_digest = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    let response = client
        .head(format!("{}/v2/test/blobs/{}", server.url(), blob_digest))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn test_docker_connectivity() {
    let docker = Docker::connect_with_local_defaults();
    assert!(docker.is_ok(), "Docker daemon not available");

    let docker = docker.unwrap();
    let info = docker.info().await;
    assert!(info.is_ok(), "Cannot connect to Docker daemon");
}

#[tokio::test]
async fn test_manifest_upload() {
    let config = RegistryConfig::memory().with_port(0);
    let server = RegistryServer::new(config).await.unwrap();

    let client = reqwest::Client::new();

    let manifest = r#"{
        "schemaVersion": 2,
        "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
        "config": {
            "mediaType": "application/vnd.docker.container.image.v1+json",
            "size": 1,
            "digest": "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        },
        "layers": []
    }"#;

    let response = client
        .put(format!("{}/v2/test/manifests/latest", server.url()))
        .header(
            "Content-Type",
            "application/vnd.docker.distribution.manifest.v2+json",
        )
        .body(manifest)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 201);

    let response = client
        .get(format!("{}/v2/test/manifests/latest", server.url()))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_docker_push_pull() {
    let config = RegistryConfig::memory();
    let server = RegistryServer::new(config).await.unwrap();
    let port = server.port();

    let docker = Docker::connect_with_local_defaults().unwrap();

    let base_image = "busybox:latest";

    let mut pull_stream = docker.create_image(
        Some(CreateImageOptions {
            from_image: Some(base_image.to_string()),
            ..Default::default()
        }),
        None,
        None,
    );

    while let Some(result) = pull_stream.next().await {
        result.unwrap();
    }

    let registry_image = format!("localhost:{}/test-image:latest", port);

    docker
        .tag_image(
            base_image,
            Some(TagImageOptions {
                repo: Some(format!("localhost:{}/test-image", port)),
                tag: Some("latest".to_string()),
            }),
        )
        .await
        .unwrap();

    let mut push_stream =
        docker.push_image(&registry_image, None::<PushImageOptions>, None);

    while let Some(result) = push_stream.next().await {
        let info = result.unwrap();
        if let Some(error) = info.error {
            panic!("Push error: {}", error);
        }
    }

    docker
        .remove_image(&registry_image, Some(RemoveImageOptions::default()), None)
        .await
        .unwrap();

    let mut pull_stream = docker.create_image(
        Some(bollard::query_parameters::CreateImageOptions {
            from_image: Some(registry_image.clone()),
            ..Default::default()
        }),
        None,
        None,
    );

    while let Some(result) = pull_stream.next().await {
        let info = result.unwrap();
        if let Some(error) = info.error {
            panic!("Pull error: {}", error);
        }
    }

    let images = docker.list_images(None::<ListImagesOptions>).await.unwrap();

    assert!(
        images
            .iter()
            .any(|img| { img.repo_tags.iter().any(|tag| tag == &registry_image) }),
        "Image not found in docker images"
    );
}
