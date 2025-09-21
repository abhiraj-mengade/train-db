#!/bin/bash

# Demo script to show connected peers in the dashboard

set -e

echo "🔗 Train-DB Connected Peers Demo"
echo "================================="
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
PURPLE='\033[0;35m'
NC='\033[0m'

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_demo() {
    echo -e "${PURPLE}[DEMO]${NC} $1"
}

# Clean up function
cleanup() {
    print_status "Stopping all Train-DB processes..."
    pkill -f "train-db" 2>/dev/null || true
    sleep 2
}

trap cleanup EXIT

# Build the project
print_status "Building Train-DB..."
cargo build --features bluetooth --quiet

# Create test directories
print_status "Setting up test directories..."
rm -rf ./demo-peers-1 ./demo-peers-2
mkdir -p ./demo-peers-1 ./demo-peers-2

print_demo "🎯 Starting Dashboard with Connected Peers"
echo "=============================================="

# Start dashboard
print_status "Starting web dashboard on port 3000..."
cargo run --features bluetooth -- --db-path ./demo-peers-1 dashboard --port 3000 > dashboard.log 2>&1 &
DASHBOARD_PID=$!
sleep 3

print_success "Dashboard started at http://localhost:3000"

# Start first P2P node
print_status "Starting P2P node 1 on port 8082..."
cargo run --features bluetooth -- --db-path ./demo-peers-1 start --port 8082 > node1.log 2>&1 &
NODE1_PID=$!
sleep 3

# Start second P2P node
print_status "Starting P2P node 2 on port 8083..."
cargo run --features bluetooth -- --db-path ./demo-peers-2 start --port 8083 > node2.log 2>&1 &
NODE2_PID=$!
sleep 3

# Let nodes run and potentially discover each other
print_status "Letting nodes run for 15 seconds to discover each other..."
sleep 15

# Check if nodes are running
if kill -0 $NODE1_PID 2>/dev/null && kill -0 $NODE2_PID 2>/dev/null; then
    print_success "Both P2P nodes are running!"
    
    # Show node logs
    print_status "Node 1 logs (last 5 lines):"
    tail -5 node1.log
    echo ""
    print_status "Node 2 logs (last 5 lines):"
    tail -5 node2.log
    echo ""
    
    print_success "🎉 Demo is running!"
    print_status "🌐 Open http://localhost:3000 in your browser"
    print_status "📊 The dashboard will show real-time peer connections"
    print_status "🔄 Refresh the dashboard to see updated peer information"
    echo ""
    print_status "The nodes are now running and should discover each other."
    print_status "Check the dashboard to see connected peers!"
    echo ""
    print_status "Press Ctrl+C to stop the demo."
    
    # Keep running
    wait
else
    print_status "Some nodes failed to start. Check the logs:"
    echo "Node 1 log:"
    cat node1.log
    echo ""
    echo "Node 2 log:"
    cat node2.log
fi
