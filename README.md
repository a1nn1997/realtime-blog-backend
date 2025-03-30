# Realtime Blog Backend

A Rust-based realtime blog backend with automatic dependency validation and seamless CI/CD integration, now with Podman support.

## Features

- Axum web framework with WebSocket support
- PostgreSQL for persistent storage
- Redis for caching and pub/sub
- Automated dependency validation and testing
- Multi-stage Podman builds
- Resource-optimized containers

## Development Setup

### Prerequisites

- Podman and Podman Compose
- Git

### Running the Development Environment

```bash
# Start the development environment with automatic code reloading
podman-compose -f src/podman-compose.yml -f src/podman-compose.dev.yml up backend

# Run tests
podman-compose -f src/podman-compose.yml -f src/podman-compose.dev.yml up test

# Generate code coverage
podman-compose -f src/podman-compose.yml -f src/podman-compose.dev.yml up coverage

# Validate dependencies explicitly
podman-compose -f src/podman-compose.yml -f src/podman-compose.dev.yml up deps-check
```

### Production Deployment

```bash
# Start the production environment
podman-compose -f src/podman-compose.yml up
```

## Dependency Management

The project includes automatic dependency validation and testing to ensure a seamless CI/CD experience:

1. **On Container Start**: The entrypoint script validates all Rust dependencies
2. **Checksum Tracking**: Cargo.toml and Cargo.lock checksums are tracked to detect changes
3. **Automatic Updates**: If dependencies change, they are automatically fetched
4. **Version Locking**: All cargo tools are installed with specific versions for consistency
5. **Security Scanning**: Dependencies are scanned for security vulnerabilities

### Customizing Dependency Validation

Set these environment variables to control dependency validation:

- `VALIDATE_DEPS_STARTUP`: Set to `false` to disable startup validation
- `CARGO_AUDIT_LEVEL`: Set to `low`, `medium`, `high`, or `critical` for security scanning
- `CARGO_FETCH_RETRIES`: Number of retries for cargo fetch operations

## CI/CD Integration

For CI/CD pipelines, use the `testing` target and the `deps-check` service:

```bash
# In your CI pipeline
podman-compose -f src/podman-compose.yml -f src/podman-compose.dev.yml up --exit-code-from deps-check deps-check
```

## Resource Optimization

All containers include resource constraints appropriate for their usage:

- **Production**: Limited to 1 CPU core and 512MB memory
- **Development**: More resources allocated for faster compilation
- **Testing/CI**: Balanced resources for reliable test execution

Adjust these limits in the `podman-compose.yml` and `podman-compose.dev.yml` files as needed.

## Docker vs. Podman

This project supports both Docker and Podman. To use Docker instead of Podman:

```bash
# Use docker-compose instead of podman-compose
docker-compose -f src/docker-compose.yml -f src/docker-compose.dev.yml up backend
```
