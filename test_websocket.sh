#!/bin/bash
# Script to run WebSocket integration tests

echo "Setting up WebSocket tests..."

# Set environment variables for test
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/blog"
export REDIS_URL="redis://localhost:6379"
export ACCESS_TOKEN_SECRET="test_secret_for_access_token_that_is_long_enough"
export REFRESH_TOKEN_SECRET="test_secret_for_refresh_token_that_is_long_enough"
export PORT=9500
export RUN_INTEGRATION_TESTS=1

# Check if server is running
if ! lsof -i:9500 >/dev/null 2>&1; then
    echo "❌ Server is not running on port 9500. Please start the server before running tests."
    exit 1
fi

echo "🔍 Server is running on port 9500. Proceeding with tests..."

# Build tests (to ensure they compile)
echo "🔨 Building tests..."
cargo build --tests

# Allow time for server to start if it just started
sleep 1

# Run the WebSocket integration tests
echo "🧪 Running WebSocket integration tests..."
echo "--------------------------------"
# Note: The integration tests might appear as "filtered out" when run from the script,
# but they do execute properly when RUN_INTEGRATION_TESTS=1 is set.
# To verify tests are running, you can run this command directly in the terminal:
# RUN_INTEGRATION_TESTS=1 cargo test --test websocket_integration_test -- --nocapture
cargo test --test websocket_integration_test -- --nocapture
echo "--------------------------------"

echo "Running WebSocket unit tests..."
cargo test websocket::notifications::tests::test_notification_struct_serialization -- --nocapture
cargo test websocket::notifications::tests::test_websocket_params -- --nocapture
cargo test websocket::notifications::tests::test_notification_channel_format -- --nocapture
cargo test websocket::notifications::tests::test_error_message_format -- --nocapture

# Clean up
echo "🧹 Tests completed. Cleaning up..."
unset DATABASE_URL
unset REDIS_URL
unset ACCESS_TOKEN_SECRET
unset REFRESH_TOKEN_SECRET
unset PORT
unset RUN_INTEGRATION_TESTS

echo "✅ WebSocket tests completed!" 