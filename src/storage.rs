use crate::error::{RegistryError, Result};
use crate::config::StorageBackend;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct ManifestEntry {
    pub data: Vec<u8>,
    pub content_type: String,
}

#[async_trait]
pub trait Storage: Send + Sync {
    async fn store_manifest(&self, key: String, entry: ManifestEntry) -> Result<()>;
    async fn get_manifest(&self, key: &str) -> Result<Option<ManifestEntry>>;
    async fn store_blob(&self, digest: String, data: Vec<u8>) -> Result<()>;
    async fn get_blob(&self, digest: &str) -> Result<Option<Vec<u8>>>;
    async fn create_upload(&self, uuid: String) -> Result<()>;
    async fn append_upload(&self, uuid: &str, data: &[u8]) -> Result<()>;
    async fn finish_upload(&self, uuid: &str) -> Result<Option<Vec<u8>>>;
}

#[derive(Default)]
pub struct MemoryStorage {
    manifests: Arc<RwLock<HashMap<String, ManifestEntry>>>,
    blobs: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    uploads: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl Storage for MemoryStorage {
    async fn store_manifest(&self, key: String, entry: ManifestEntry) -> Result<()> {
        self.manifests.write().await.insert(key, entry);
        Ok(())
    }

    async fn get_manifest(&self, key: &str) -> Result<Option<ManifestEntry>> {
        Ok(self.manifests.read().await.get(key).cloned())
    }

    async fn store_blob(&self, digest: String, data: Vec<u8>) -> Result<()> {
        self.blobs.write().await.insert(digest, data);
        Ok(())
    }

    async fn get_blob(&self, digest: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.blobs.read().await.get(digest).cloned())
    }

    async fn create_upload(&self, uuid: String) -> Result<()> {
        self.uploads.write().await.insert(uuid, Vec::new());
        Ok(())
    }

    async fn append_upload(&self, uuid: &str, data: &[u8]) -> Result<()> {
        if let Some(upload) = self.uploads.write().await.get_mut(uuid) {
            upload.extend_from_slice(data);
            Ok(())
        } else {
            Err(RegistryError::UploadNotFound(uuid.to_string()))
        }
    }

    async fn finish_upload(&self, uuid: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.uploads.write().await.remove(uuid))
    }
}

pub struct DiskStorage {
    base_path: PathBuf,
    _temp_dir: Option<tempfile::TempDir>,
}

impl DiskStorage {
    pub async fn new(path: PathBuf) -> Result<Self> {
        fs::create_dir_all(&path).await?;
        fs::create_dir_all(path.join("manifests")).await?;
        fs::create_dir_all(path.join("blobs")).await?;
        fs::create_dir_all(path.join("uploads")).await?;
        
        Ok(Self {
            base_path: path,
            _temp_dir: None,
        })
    }

    pub async fn temp() -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().to_path_buf();
        
        fs::create_dir_all(path.join("manifests")).await?;
        fs::create_dir_all(path.join("blobs")).await?;
        fs::create_dir_all(path.join("uploads")).await?;
        
        Ok(Self {
            base_path: path,
            _temp_dir: Some(temp_dir),
        })
    }

    fn manifest_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace(['/', ':'], "_");
        self.base_path.join("manifests").join(format!("{}.json", safe_key))
    }

    fn manifest_meta_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace(['/', ':'], "_");
        self.base_path.join("manifests").join(format!("{}.meta", safe_key))
    }

    fn blob_path(&self, digest: &str) -> PathBuf {
        let safe_digest = digest.replace(['/', ':'], "_");
        self.base_path.join("blobs").join(safe_digest)
    }

    fn upload_path(&self, uuid: &str) -> PathBuf {
        self.base_path.join("uploads").join(uuid)
    }
}

#[async_trait]
impl Storage for DiskStorage {
    async fn store_manifest(&self, key: String, entry: ManifestEntry) -> Result<()> {
        let manifest_path = self.manifest_path(&key);
        let meta_path = self.manifest_meta_path(&key);
        
        fs::write(&manifest_path, &entry.data).await?;
        fs::write(&meta_path, &entry.content_type).await?;
        
        Ok(())
    }

    async fn get_manifest(&self, key: &str) -> Result<Option<ManifestEntry>> {
        let manifest_path = self.manifest_path(key);
        let meta_path = self.manifest_meta_path(key);
        
        if !manifest_path.exists() {
            return Ok(None);
        }
        
        let data = fs::read(&manifest_path).await?;
        let content_type = fs::read_to_string(&meta_path).await
            .unwrap_or_else(|_| "application/vnd.docker.distribution.manifest.v2+json".to_string());
        
        Ok(Some(ManifestEntry { data, content_type }))
    }

    async fn store_blob(&self, digest: String, data: Vec<u8>) -> Result<()> {
        let blob_path = self.blob_path(&digest);
        fs::write(&blob_path, &data).await?;
        Ok(())
    }

    async fn get_blob(&self, digest: &str) -> Result<Option<Vec<u8>>> {
        let blob_path = self.blob_path(digest);
        
        if !blob_path.exists() {
            return Ok(None);
        }
        
        let data = fs::read(&blob_path).await?;
        Ok(Some(data))
    }

    async fn create_upload(&self, uuid: String) -> Result<()> {
        let upload_path = self.upload_path(&uuid);
        fs::write(&upload_path, &[]).await?;
        Ok(())
    }

    async fn append_upload(&self, uuid: &str, data: &[u8]) -> Result<()> {
        let upload_path = self.upload_path(uuid);
        
        if !upload_path.exists() {
            return Err(RegistryError::UploadNotFound(uuid.to_string()));
        }
        
        let mut existing = fs::read(&upload_path).await?;
        existing.extend_from_slice(data);
        fs::write(&upload_path, &existing).await?;
        
        Ok(())
    }

    async fn finish_upload(&self, uuid: &str) -> Result<Option<Vec<u8>>> {
        let upload_path = self.upload_path(uuid);
        
        if !upload_path.exists() {
            return Ok(None);
        }
        
        let data = fs::read(&upload_path).await?;
        fs::remove_file(&upload_path).await?;
        
        Ok(Some(data))
    }
}

pub async fn create_storage(backend: &StorageBackend) -> Result<Arc<dyn Storage>> {
    match backend {
        StorageBackend::Memory => Ok(Arc::new(MemoryStorage::new())),
        StorageBackend::TempDir => Ok(Arc::new(DiskStorage::temp().await?)),
        StorageBackend::Directory(path) => Ok(Arc::new(DiskStorage::new(path.clone()).await?)),
    }
}
