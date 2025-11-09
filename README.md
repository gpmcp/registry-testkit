# registry-testkit

A minimal, OCI-compliant container registry library for testing and development.

## Features

- Docker Registry HTTP API V2 implementation
- OCI Distribution Specification support
- Two storage backends: in-memory and filesystem
- Automatic port selection or manual configuration
- Zero-copy blob operations
- Manifest content-type preservation
- Compatible with Docker, Podman, and other OCI tools

## Usage

### As a Library

```rust
use registry_testkit::{RegistryConfig, RegistryServer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // In-memory storage with auto port
    let config = RegistryConfig::memory();
    let server = RegistryServer::new(config).await?;
    println!("Registry: {}", server.url());

    // Or with specific port
    let config = RegistryConfig::memory().with_port(5000);
    let server = RegistryServer::new(config).await?;

    // Or with filesystem storage
    let config = RegistryConfig::temp_dir();
    let server = RegistryServer::new(config).await?;

    Ok(())
}
```

### Examples

```bash
# Simple in-memory registry
cargo run --example simple

# Filesystem-backed registry
cargo run --example temp_dir
```

## Testing with Docker

```bash
# Build and push
docker build -t localhost:5000/myapp:v1 .
docker push localhost:5000/myapp:v1

# Pull
docker pull localhost:5000/myapp:v1
```

## Testing with Podman

```bash
# Build and push
podman build -t localhost:5000/myapp:v1 .
podman push --tls-verify=false localhost:5000/myapp:v1

# Pull
podman pull --tls-verify=false localhost:5000/myapp:v1
```

## API

### `RegistryConfig`

```rust
// Create with in-memory storage
let config = RegistryConfig::memory();

// Create with temp directory storage
let config = RegistryConfig::temp_dir();

// Set specific port (default: auto-select)
let config = config.with_port(5000);

// Set host (default: 0.0.0.0)
let config = config.with_host("127.0.0.1");
```

### `RegistryServer`

```rust
// Create server
let server = RegistryServer::new(config).await?;

// Get URL
let url = server.url(); // e.g., "http://0.0.0.0:5000"

// Get port
let port = server.port(); // e.g., 5000
```

## Supported Manifest Types

- `application/vnd.docker.distribution.manifest.v2+json` (Docker V2)
- `application/vnd.docker.distribution.manifest.list.v2+json` (Docker Manifest List)
- `application/vnd.oci.image.manifest.v1+json` (OCI Image Manifest)
- `application/vnd.oci.image.index.v1+json` (OCI Image Index)

## License

MIT OR Apache-2.0
