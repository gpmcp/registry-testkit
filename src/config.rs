//! Configuration types for the registry server.

use std::path::PathBuf;

/// Storage backend for registry data.
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// In-memory storage (data lost when server stops).
    Memory,
    /// Temporary directory storage (cleaned up automatically).
    TempDir,
    /// Persistent directory storage at a specific path.
    Directory(PathBuf),
}

/// Configuration for the registry server.
#[derive(Debug, Clone)]
pub struct RegistryConfig {
    /// Storage backend to use.
    pub storage: StorageBackend,
    /// Port to bind to (None for random port).
    pub port: Option<u16>,
    /// Host address to bind to.
    pub host: String,
}

impl RegistryConfig {
    /// Creates a new configuration with the specified storage backend.
    pub fn new(storage: StorageBackend) -> Self {
        Self {
            storage,
            port: None,
            host: "127.0.0.1".to_string(),
        }
    }

    /// Creates a configuration with in-memory storage.
    pub fn memory() -> Self {
        Self::new(StorageBackend::Memory)
    }

    /// Creates a configuration with temporary directory storage.
    pub fn temp_dir() -> Self {
        Self::new(StorageBackend::TempDir)
    }

    /// Creates a configuration with directory storage at the specified path.
    pub fn directory(path: PathBuf) -> Self {
        Self::new(StorageBackend::Directory(path))
    }

    /// Sets a specific port for the server to bind to.
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Sets the host address for the server to bind to.
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
