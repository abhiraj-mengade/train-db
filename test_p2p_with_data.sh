#!/bin/bash

# P2P Gossiping Test with Data
# Start two nodes, connect them, set data, let them gossip, then check

set -e

echo "🚀 Testing P2P Gossiping with Data"
echo "=================================="

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

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
    print_status "Stopping nodes..."
    pkill -f "train-db.*start" 2>/dev/null || true
    sleep 2
}

trap cleanup EXIT

# Create test directories
print_status "Setting up test directories..."
rm -rf ./test-p2p-1 ./test-p2p-2
mkdir -p ./test-p2p-1 ./test-p2p-2

# Build the project
print_status "Building Train-DB..."
cargo build --features bluetooth --quiet

# Start first node
print_status "Starting node 1 on port 8082..."
cargo run --features bluetooth -- --db-path ./test-p2p-1 start --port 8082 > node1.log 2>&1 &
NODE1_PID=$!
sleep 3

# Start second node  
print_status "Starting node 2 on port 8083..."
cargo run --features bluetooth -- --db-path ./test-p2p-2 start --port 8083 > node2.log 2>&1 &
NODE2_PID=$!
sleep 3

# Let nodes initialize
print_status "Letting nodes initialize..."
sleep 5

# Stop the nodes
print_status "Stopping nodes to set initial data..."
kill $NODE1_PID $NODE2_PID 2>/dev/null || true
sleep 2

# Set some initial data on node 1
print_status "Setting initial data on node 1..."
cargo run --features bluetooth -- --db-path ./test-p2p-1 set "test-key-1" "Hello from Node 1" > /dev/null 2>&1
cargo run --features bluetooth -- --db-path ./test-p2p-1 set "shared-key" "Initial value" > /dev/null 2>&1

# Set some initial data on node 2
print_status "Setting initial data on node 2..."
cargo run --features bluetooth -- --db-path ./test-p2p-2 set "test-key-2" "Hello from Node 2" > /dev/null 2>&1
cargo run --features bluetooth -- --db-path ./test-p2p-2 set "shared-key" "Updated by Node 2" > /dev/null 2>&1

# Show initial state
print_status "Initial state:"
echo "Node 1 data:"
cargo run --features bluetooth -- --db-path ./test-p2p-1 list 2>/dev/null || echo "No data"
echo ""
echo "Node 2 data:"
cargo run --features bluetooth -- --db-path ./test-p2p-2 list 2>/dev/null || echo "No data"

# Start nodes again
print_status "Starting nodes again for P2P gossiping..."
cargo run --features bluetooth -- --db-path ./test-p2p-1 start --port 8082 > node1.log 2>&1 &
NODE1_PID=$!
sleep 3

cargo run --features bluetooth -- --db-path ./test-p2p-2 start --port 8083 > node2.log 2>&1 &
NODE2_PID=$!
sleep 3

# Let them run and potentially gossip
print_status "Letting nodes run for 15 seconds to discover and gossip..."
sleep 15

# Stop the nodes
print_status "Stopping nodes to check final state..."
kill $NODE1_PID $NODE2_PID 2>/dev/null || true
sleep 2

# Check final state
print_status "Final state after gossiping:"
echo ""
echo "Node 1 data:"
cargo run --features bluetooth -- --db-path ./test-p2p-1 list 2>/dev/null || echo "No data"
echo ""
echo "Node 2 data:"
cargo run --features bluetooth -- --db-path ./test-p2p-2 list 2>/dev/null || echo "No data"

# Show some logs
echo ""
print_status "Node 1 logs (last 10 lines):"
tail -10 node1.log 2>/dev/null || echo "No logs"

echo ""
print_status "Node 2 logs (last 10 lines):"
tail -10 node2.log 2>/dev/null || echo "No logs"

print_success "P2P gossiping test completed!"
