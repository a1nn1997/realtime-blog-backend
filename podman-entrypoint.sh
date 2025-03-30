#!/bin/bash
set -e

echo "🔍 Validating Rust environment and dependencies..."

# Verify Rust and cargo are working
rustc --version
cargo --version

# Check if dependencies need updating
if [ -f "Cargo.toml" ]; then
    echo "📦 Verifying Cargo dependencies..."
    
    # Save current dependencies to a checksum file if it doesn't exist
    if [ ! -f ".cargo_checksum" ]; then
        sha256sum Cargo.toml Cargo.lock > .cargo_checksum
    fi
    
    # Check if dependencies have changed
    if ! sha256sum -c .cargo_checksum &>/dev/null; then
        echo "🔄 Cargo dependencies have changed, updating..."
        # Run in offline mode first to use cache, then online if needed
        cargo fetch --locked || cargo fetch
        sha256sum Cargo.toml Cargo.lock > .cargo_checksum
    else
        echo "✅ Cargo dependencies verified and up to date."
    fi
    
    # Verify build works
    echo "🔨 Verifying build compilation..."
    cargo check --quiet || cargo check
    
    # In development environment, check that testing tools work
    if [ "${ENVIRONMENT:-production}" = "development" ]; then
        echo "🧪 Verifying test utilities..."
        if command -v cargo-tarpaulin &>/dev/null; then
            echo "✅ cargo-tarpaulin is available"
        else
            echo "⚠️ cargo-tarpaulin is missing, installing..."
            cargo install cargo-tarpaulin
        fi
        
        if command -v cargo-nextest &>/dev/null; then
            echo "✅ cargo-nextest is available"
        else
            echo "⚠️ cargo-nextest is missing, installing..."
            cargo install cargo-nextest
        fi
        
        if command -v cargo-audit &>/dev/null; then
            echo "✅ cargo-audit is available"
            # Run a security audit in development environments
            cargo audit --quiet || echo "⚠️ Security warnings detected"
        else
            echo "⚠️ cargo-audit is missing, installing..."
            cargo install cargo-audit
        fi
    fi
fi

echo "✅ Environment validation complete!"

# Execute the command passed to podman run
exec "$@" 