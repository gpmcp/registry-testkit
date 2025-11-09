use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum StorageBackend {
    Memory,
    TempDir,
    Directory(PathBuf),
}

#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub storage: StorageBackend,
    pub port: Option<u16>,
    pub host: String,
}

impl RegistryConfig {
    pub fn new(storage: StorageBackend) -> Self {
        Self {
            storage,
            port: None,
            host: "127.0.0.1".to_string(),
        }
    }

    pub fn memory() -> Self {
        Self::new(StorageBackend::Memory)
    }

    pub fn temp_dir() -> Self {
        Self::new(StorageBackend::TempDir)
    }

    pub fn directory(path: PathBuf) -> Self {
        Self::new(StorageBackend::Directory(path))
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self::memory()
    }
}
