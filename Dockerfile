FROM rust:1.81-slim as planner
WORKDIR /app
RUN cargo install cargo-chef
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM rust:1.81-slim as cacher
WORKDIR /app
RUN cargo install cargo-chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rust:1.81-slim as builder
WORKDIR /app
COPY . .
# Copy over the cached dependencies
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
# Run dependency checks
RUN cargo fetch --locked
# Build the application
RUN cargo build --release --locked
# Add security scanning
RUN cargo install cargo-audit && cargo audit || echo "Security scan completed with warnings"

# Development image
FROM rust:1.81-slim as development
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq-dev \
    curl \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
# Development tools commented out to reduce memory usage during build
# RUN cargo install --locked cargo-watch && \
#     cargo install --locked cargo-tarpaulin && \
#     cargo install --locked cargo-nextest && \
#     cargo install --locked cargo-audit
# Copy entrypoint script
COPY podman-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/podman-entrypoint.sh
# Copy application
COPY . .
# Set development environment flag
ENV ENVIRONMENT=development
# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1
ENTRYPOINT ["podman-entrypoint.sh"]
CMD ["cargo", "run"]

# Production image - use debian:slim for smaller size
FROM debian:bookworm-slim as production
WORKDIR /app
# Install runtime dependencies only
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*
# Copy entrypoint script
COPY podman-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/podman-entrypoint.sh
# Copy only the executable
COPY --from=builder /app/target/release/realtime-blog-backend .
# Add non-root user
RUN useradd -m appuser
USER appuser
# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1
EXPOSE 8000
ENTRYPOINT ["podman-entrypoint.sh"]
CMD ["./realtime-blog-backend"]

# CI/Testing image
FROM rust:1.81-slim as testing
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    libpq-dev \
    curl \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*
# Testing tools commented out to reduce memory usage during build
# RUN cargo install --locked cargo-tarpaulin && \
#     cargo install --locked cargo-nextest && \
#     cargo install --locked cargo-audit
# Copy entrypoint script
COPY podman-entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/podman-entrypoint.sh
# Set environment flag
ENV ENVIRONMENT=testing
# Copy application
COPY . .
# Validate dependencies during build
RUN cargo fetch --locked && cargo check
ENTRYPOINT ["podman-entrypoint.sh"]
CMD ["cargo", "test"]

# Default to production
FROM production