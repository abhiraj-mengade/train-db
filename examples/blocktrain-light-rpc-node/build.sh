#!/bin/bash

# Build script for lightweight-rpc-node

set -e

echo "🔨 Building Lightweight RPC Node..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Cargo not found. Please install Rust: https://rustup.rs/"
    exit 1
fi

# Build with Bluetooth support by default
echo "📦 Building with Bluetooth support..."
cargo build --release --features bluetooth

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo ""
    echo "🚀 To run the node:"
    echo "   ./target/release/rpc-node"
    echo ""
    echo "📋 To generate config:"
    echo "   ./target/release/rpc-node generate-config"
    echo ""
    echo "🔍 To check config:"
    echo "   ./target/release/rpc-node check-config"
else
    echo "❌ Build failed"
    exit 1
fi
