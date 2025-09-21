#!/bin/bash

# Train-DB Auto-Mesh Demo Script
# This script demonstrates the intelligent multi-transport mesh networking

set -e

echo "🚂 Train-DB Auto-Mesh Demo"
echo "=========================="
echo ""
echo "This demo showcases the intelligent multi-transport mesh networking"
echo "that automatically switches between Wi-Fi and Bluetooth transports"
echo "to maintain connectivity in hostile network conditions."
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
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

print_demo() {
    echo -e "${PURPLE}[DEMO]${NC} $1"
}

print_mesh() {
    echo -e "${CYAN}[MESH]${NC} $1"
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    print_error "Please run this script from the train-db project root directory"
    exit 1
fi

print_status "Building Train-DB with Bluetooth support..."
if cargo build --features bluetooth --quiet; then
    print_success "Build completed successfully"
else
    print_error "Build failed. Make sure you have all dependencies installed."
    exit 1
fi

echo ""
print_demo "🎬 Starting Auto-Mesh Demo"
echo "================================"
echo ""
print_mesh "The Auto-Mesh Node will:"
print_mesh "  • Start both Wi-Fi and Bluetooth transports"
print_mesh "  • Continuously scan for peers on both networks"
print_mesh "  • Automatically switch to Bluetooth when Wi-Fi fails"
print_mesh "  • Show real-time mesh quality metrics"
print_mesh "  • Demonstrate intelligent routing decisions"
echo ""

# Check Bluetooth status on macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    print_status "Checking macOS Bluetooth status..."
    if system_profiler SPBluetoothDataType | grep -q "Bluetooth Power: On"; then
        print_success "Bluetooth is enabled on this system"
    else
        print_warning "Bluetooth may not be enabled. The demo will still work but may show limited Bluetooth functionality."
    fi
fi

echo ""
print_demo "🚀 Launching Auto-Mesh Node (30-second demo)..."
echo ""

# Run the mesh demo
if cargo run --features bluetooth mesh-demo --duration 30; then
    echo ""
    print_success "✅ Auto-Mesh Demo completed successfully!"
else
    print_error "❌ Auto-Mesh Demo failed"
    exit 1
fi

echo ""
print_demo "🎯 Demo Results Analysis"
echo "============================"
echo ""
print_mesh "What you just saw:"
print_mesh "  • Multi-transport initialization (Wi-Fi + Bluetooth)"
print_mesh "  • Continuous peer discovery on both transports"
print_mesh "  • Transport health monitoring and failure detection"
print_mesh "  • Automatic fallback to alternative transports"
print_mesh "  • Real-time mesh quality assessment"
echo ""

print_demo "🔧 Key Features Demonstrated:"
echo "=================================="
echo ""
print_success "✅ Multi-Transport Support"
print_mesh "   - Simultaneous Wi-Fi and Bluetooth operation"
print_mesh "   - Transport-agnostic peer discovery"
echo ""
print_success "✅ Intelligent Transport Switching"
print_mesh "   - Automatic detection of transport failures"
print_mesh "   - Seamless fallback to alternative transports"
print_mesh "   - Real-time transport health monitoring"
echo ""
print_success "✅ Mesh Quality Assessment"
print_mesh "   - Connection quality metrics (0.0 to 1.0)"
print_mesh "   - Transport diversity scoring"
print_mesh "   - Overall mesh health evaluation"
echo ""
print_success "✅ Fault Tolerance"
print_mesh "   - Graceful handling of network interruptions"
print_mesh "   - Continuous operation despite transport failures"
print_mesh "   - Automatic recovery when transports come back online"
echo ""

print_demo "🚂 Train Network Scenario"
echo "============================="
echo ""
print_mesh "In a real train environment, this system would:"
print_mesh "  • Detect when Wi-Fi hotspots become unavailable"
print_mesh "  • Automatically switch to Bluetooth mesh networking"
print_mesh "  • Maintain data synchronization across all connected devices"
print_mesh "  • Provide seamless connectivity as passengers move between cars"
print_mesh "  • Handle intermittent connectivity gracefully"
echo ""

print_demo "🔬 Technical Implementation"
echo "==============================="
echo ""
print_mesh "The auto-mesh system uses:"
print_mesh "  • libp2p for P2P networking abstraction"
print_mesh "  • btleplug for cross-platform Bluetooth support"
print_mesh "  • RocksDB for persistent key-value storage"
print_mesh "  • Async Rust for concurrent transport management"
print_mesh "  • Intelligent routing algorithms for message forwarding"
echo ""

print_demo "🎮 Try It Yourself!"
echo "======================"
echo ""
print_status "You can run the demo with different durations:"
echo "  cargo run --features bluetooth mesh-demo --duration 60  # 1 minute"
echo "  cargo run --features bluetooth mesh-demo --duration 120 # 2 minutes"
echo ""
print_status "Or test individual components:"
echo "  cargo run --features bluetooth test-bluetooth           # Bluetooth only"
echo "  cargo run api --port 8080                              # HTTP API"
echo ""

print_success "🎉 Auto-Mesh Demo Complete!"
print_mesh "Your Train-DB now has intelligent multi-transport mesh networking!"
echo ""
