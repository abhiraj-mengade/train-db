#!/bin/bash

# Demo script to showcase the clean, elegant dashboard

set -e

echo "🎨 Train-DB Clean Dashboard Demo"
echo "================================"
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
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
    print_status "Stopping Train-DB processes..."
    pkill -f "train-db" 2>/dev/null || true
    sleep 2
}

trap cleanup EXIT

# Build the project
print_status "Building Train-DB with clean UI..."
cargo build --features bluetooth --quiet

# Create test directory
print_status "Setting up test directory..."
rm -rf ./demo-dashboard
mkdir -p ./demo-dashboard

print_demo "🎯 Starting Clean Dashboard"
echo "============================="

# Start dashboard with integrated P2P node
print_status "Starting integrated P2P node with dashboard..."
cargo run --features bluetooth -- --db-path ./demo-dashboard node-with-dashboard --node-port 8082 --dashboard-port 3000 > dashboard.log 2>&1 &
DASHBOARD_PID=$!
sleep 5

print_success "Dashboard started successfully!"

# Test the dashboard API
print_status "Testing dashboard API..."
DASHBOARD_RESPONSE=$(curl -s http://localhost:3000/api/dashboard)
if echo "${DASHBOARD_RESPONSE}" | grep -q "node_stats"; then
    print_success "Dashboard API is working"
    echo ""
    print_status "Dashboard Data:"
    echo "${DASHBOARD_RESPONSE}" | jq '.node_stats'
    echo ""
    print_status "Recent Activity:"
    echo "${DASHBOARD_RESPONSE}" | jq '.recent_activity'
else
    echo "Dashboard API test failed"
    exit 1
fi

print_success "🎉 Clean Dashboard Demo Complete!"
echo ""
print_status "🌐 Dashboard available at: http://localhost:3000"
print_status "📊 Features:"
print_status "   ✅ Clean, elegant design (no gradients)"
print_status "   ✅ Real P2P data integration"
print_status "   ✅ Live activity monitoring"
print_status "   ✅ Professional dark theme"
print_status "   ✅ Auto-refresh every 5 seconds"
echo ""
print_status "🔄 The dashboard shows real-time data from the P2P node"
print_status "📱 Perfect for monitoring Train-DB in production"
echo ""
print_status "Press Ctrl+C to stop the demo."

# Keep running
wait
