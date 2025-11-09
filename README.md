# registry-testkit

A minimal, OCI-compliant container registry mock for testing.

## Usage

Start a registry in your tests:

```rust
use registry_testkit::{RegistryConfig, RegistryServer};

#[tokio::test]
async fn test_docker_operations() {
    // Start registry with auto-assigned port
    let config = RegistryConfig::memory();
    let server = RegistryServer::new(config).await.unwrap();
    
    // Use in your tests
    let registry_url = server.url();
    let port = server.port();
    
    // Test your Docker/OCI operations
    // ...
}
```

## Configuration

```rust
// In-memory storage (fast, for tests)
let config = RegistryConfig::memory();

// Filesystem storage (persistent)
let config = RegistryConfig::temp_dir();

// Custom port
let config = RegistryConfig::memory().with_port(5000);

// Custom host
let config = RegistryConfig::memory().with_host("127.0.0.1");
```

## Example Tests

See [tests/integration_test.rs](tests/integration_test.rs) for complete examples:

```rust
use bollard::Docker;
use bollard::image::{PushImageOptions, TagImageOptions};
use futures_util::stream::StreamExt;

#[tokio::test]
async fn test_docker_push_pull() {
    let config = RegistryConfig::memory();
    let server = RegistryServer::new(config).await.unwrap();
    
    let docker = Docker::connect_with_local_defaults().unwrap();
    let registry_image = format!("localhost:{}/test:latest", server.port());
    
    // Tag and push
    docker.tag_image("busybox:latest", Some(TagImageOptions {
        repo: format!("localhost:{}/test", server.port()),
        tag: "latest".to_string(),
    })).await.unwrap();
    
    let mut push = docker.push_image(&registry_image, None::<PushImageOptions<String>>, None);
    while let Some(result) = push.next().await {
        result.unwrap();
    }
    
    // Now pull from your mock registry
    // ...
}
```

## Features

- Docker Registry HTTP API V2
- OCI Distribution Specification support
- In-memory or filesystem storage
- Automatic port selection
- Compatible with Docker, Podman, and OCI tools

## Running Tests

```bash
cargo test
```

## License

MIT OR Apache-2.0
