use thiserror::Error;

pub type Result<T> = std::result::Result<T, RegistryError>;

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Upload not found: {0}")]
    UploadNotFound(String),
}
