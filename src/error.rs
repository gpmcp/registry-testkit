//! Error types for the registry.

use thiserror::Error;

/// Result type alias for registry operations.
pub type Result<T> = std::result::Result<T, RegistryError>;

/// Errors that can occur during registry operations.
#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Upload not found: {0}")]
    UploadNotFound(String),
}
