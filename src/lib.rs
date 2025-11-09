//! A minimal, OCI-compliant container registry for testing and development.
//!
//! This crate provides a simple way to spin up a local container registry for testing
//! Docker/OCI container workflows without external dependencies.
//!
//! # Examples
//!
//! ```no_run
//! use registry_testkit::{RegistryServer, RegistryConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = RegistryConfig::memory();
//!     let server = RegistryServer::new(config).await?;
//!     println!("Registry running at {}", server.url());
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod error;
pub mod server;
pub mod storage;

pub use config::{RegistryConfig, StorageBackend};
pub use error::{RegistryError, Result};
pub use server::RegistryServer;
