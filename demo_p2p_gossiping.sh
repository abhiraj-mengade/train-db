#!/bin/bash

# Train-DB P2P Gossiping Demo Script
# This script demonstrates the P2P gossiping functionality

set -e

echo "🚀 Train-DB P2P Gossiping Demo"
echo "================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Clean up function
cleanup() {
    print_status "Cleaning up..."
    pkill -f "train-db.*start" 2>/dev/null || true
    sleep 2
}

# Set up cleanup trap
trap cleanup EXIT

# Check if the project builds
print_status "Building Train-DB with P2P support..."
if ! cargo build --features bluetooth --quiet; then
    print_error "Failed to build Train-DB"
    exit 1
fi
print_success "Build completed successfully"

# Create test directories
print_status "Setting up test directories..."
rm -rf ./test-p2p-1 ./test-p2p-2
mkdir -p ./test-p2p-1 ./test-p2p-2
print_success "Test directories created"

# Start first P2P node
print_status "Starting first P2P node on port 8082..."
cargo run --features bluetooth -- --db-path ./test-p2p-1 start --port 8082 > node1.log 2>&1 &
NODE1_PID=$!
sleep 3

# Check if node1 is running
if ! kill -0 $NODE1_PID 2>/dev/null; then
    print_error "Failed to start first P2P node"
    cat node1.log
    exit 1
fi
print_success "First P2P node started (PID: $NODE1_PID)"

# Start second P2P node
print_status "Starting second P2P node on port 8083..."
cargo run --features bluetooth -- --db-path ./test-p2p-2 start --port 8083 > node2.log 2>&1 &
NODE2_PID=$!
sleep 3

# Check if node2 is running
if ! kill -0 $NODE2_PID 2>/dev/null; then
    print_error "Failed to start second P2P node"
    cat node2.log
    exit 1
fi
print_success "Second P2P node started (PID: $NODE2_PID)"

# Wait for nodes to initialize
print_status "Waiting for nodes to initialize and discover each other..."
sleep 5

# Test P2P functionality
print_status "Testing P2P gossiping functionality..."

# Set a key on node1
print_status "Setting key 'p2p-test' = 'Hello P2P World' on node1..."
if cargo run --features bluetooth -- --db-path ./test-p2p-1 set p2p-test "Hello P2P World" > /dev/null 2>&1; then
    print_success "Key set on node1"
else
    print_error "Failed to set key on node1"
    exit 1
fi

# Wait for gossiping
print_status "Waiting for P2P gossiping to propagate..."
sleep 3

# Check if the key exists on node2
print_status "Checking if key was synchronized to node2..."
if cargo run --features bluetooth -- --db-path ./test-p2p-2 get p2p-test > /dev/null 2>&1; then
    print_success "Key successfully synchronized to node2 via P2P gossiping!"
else
    print_warning "Key not found on node2 - P2P gossiping may need more time or manual connection"
fi

# Test another key from node2
print_status "Setting key 'p2p-test-2' = 'From Node2' on node2..."
if cargo run --features bluetooth -- --db-path ./test-p2p-2 set p2p-test-2 "From Node2" > /dev/null 2>&1; then
    print_success "Key set on node2"
else
    print_error "Failed to set key on node2"
    exit 1
fi

# Wait for gossiping
sleep 3

# Check if the key exists on node1
print_status "Checking if key was synchronized to node1..."
if cargo run --features bluetooth -- --db-path ./test-p2p-1 get p2p-test-2 > /dev/null 2>&1; then
    print_success "Key successfully synchronized to node1 via P2P gossiping!"
else
    print_warning "Key not found on node1 - P2P gossiping may need more time or manual connection"
fi

# Show final status
print_status "Final database contents:"
echo ""
print_status "Node1 database:"
cargo run --features bluetooth -- --db-path ./test-p2p-1 list 2>/dev/null || echo "No keys found"
echo ""
print_status "Node2 database:"
cargo run --features bluetooth -- --db-path ./test-p2p-2 list 2>/dev/null || echo "No keys found"

# Show logs
echo ""
print_status "Node1 logs (last 10 lines):"
tail -10 node1.log 2>/dev/null || echo "No logs available"
echo ""
print_status "Node2 logs (last 10 lines):"
tail -10 node2.log 2>/dev/null || echo "No logs available"

echo ""
print_success "P2P Gossiping Demo completed!"
print_status "Both nodes are still running. You can:"
print_status "  - Set keys on either node and see them propagate"
print_status "  - Check the logs for P2P activity"
print_status "  - Press Ctrl+C to stop the demo"

# Keep nodes running for manual testing
print_status "Nodes will continue running. Press Ctrl+C to stop..."
wait
