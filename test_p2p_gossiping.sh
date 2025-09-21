#!/bin/bash

# Simple P2P Gossiping Test
# Start two nodes, let them gossip, stop them, then check the data

set -e

echo "🚀 Testing P2P Gossiping"
echo "========================"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
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

# Let nodes run and potentially discover each other
print_status "Letting nodes run for 10 seconds to discover each other..."
sleep 10

# Stop the nodes
print_status "Stopping nodes..."
kill $NODE1_PID $NODE2_PID 2>/dev/null || true
sleep 2

# Now check the data in both databases
print_status "Checking data in both databases..."

echo ""
echo "Node 1 database contents:"
if cargo run --features bluetooth -- --db-path ./test-p2p-1 list 2>/dev/null; then
    print_success "Node 1 database accessible"
else
    echo "No data or error accessing node 1 database"
fi

echo ""
echo "Node 2 database contents:"
if cargo run --features bluetooth -- --db-path ./test-p2p-2 list 2>/dev/null; then
    print_success "Node 2 database accessible"
else
    echo "No data or error accessing node 2 database"
fi

# Show some logs
echo ""
print_status "Node 1 logs (last 5 lines):"
tail -5 node1.log 2>/dev/null || echo "No logs"

echo ""
print_status "Node 2 logs (last 5 lines):"
tail -5 node2.log 2>/dev/null || echo "No logs"

print_success "P2P test completed!"
