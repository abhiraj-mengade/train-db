#!/bin/bash

# Train-DB Multi-Transport Demo Script
# This script demonstrates the multi-transport mesh networking capabilities

set -e

echo "🚂 Train-DB Multi-Transport Demo"
echo "================================"

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Cargo not found. Please install Rust first."
    exit 1
fi

# Build the project
echo "🔨 Building Train-DB..."
cargo build --release

echo "✅ Build complete!"
echo ""

# Function to start a node
start_node() {
    local port=$1
    local name=$2
    local interactive=$3
    
    echo "🚀 Starting $name on port $port..."
    
    if [ "$interactive" = "true" ]; then
        echo "   Interactive mode enabled"
        cargo run --release -- start --port $port --interactive --verbose
    else
        echo "   Background mode"
        cargo run --release -- start --port $port --verbose &
        echo $! > "node_${port}.pid"
    fi
}

# Function to stop background nodes
stop_nodes() {
    echo "🛑 Stopping background nodes..."
    for pidfile in node_*.pid; do
        if [ -f "$pidfile" ]; then
            pid=$(cat "$pidfile")
            kill $pid 2>/dev/null || true
            rm "$pidfile"
        fi
    done
}

# Function to test basic operations
test_operations() {
    local db_path=$1
    echo "🧪 Testing basic operations..."
    
    # Test set operation
    echo "   Setting key 'demo' = 'Hello, Train-DB!'"
    cargo run --release -- --db-path "$db_path" set demo "Hello, Train-DB!"
    
    # Test get operation
    echo "   Getting key 'demo'"
    cargo run --release -- --db-path "$db_path" get demo
    
    # Test list operation
    echo "   Listing all keys"
    cargo run --release -- --db-path "$db_path" list
    
    # Test info operation
    echo "   Database info"
    cargo run --release -- --db-path "$db_path" info
}

# Function to show network info
show_network_info() {
    echo "🌐 Network Information"
    echo "   Local IP addresses:"
    if command -v ip &> /dev/null; then
        ip addr show | grep "inet " | grep -v "127.0.0.1" | awk '{print "     " $2}'
    elif command -v ifconfig &> /dev/null; then
        ifconfig | grep "inet " | grep -v "127.0.0.1" | awk '{print "     " $2}'
    fi
    
    echo "   Bluetooth status:"
    if command -v bluetoothctl &> /dev/null; then
        bluetoothctl show | grep "Powered" || echo "     Bluetooth not available"
    else
        echo "     bluetoothctl not found"
    fi
}

# Main demo function
run_demo() {
    echo "🎯 Starting Multi-Transport Demo"
    echo ""
    
    # Show network info
    show_network_info
    echo ""
    
    # Test basic operations first
    echo "📝 Testing basic operations..."
    test_operations "./demo_data"
    echo ""
    
    # Ask user what they want to do
    echo "What would you like to do?"
    echo "1) Start interactive node (recommended for testing)"
    echo "2) Start background nodes for multi-node demo"
    echo "3) Test Bluetooth functionality"
    echo "4) Exit"
    echo ""
    read -p "Enter your choice (1-4): " choice
    
    case $choice in
        1)
            echo "🚀 Starting interactive node..."
            echo "   Use commands: set <key> <value>, get <key>, list, delete <key>"
            echo "   Type 'quit' to exit"
            echo ""
            cargo run --release -- start --interactive --verbose
            ;;
        2)
            echo "🚀 Starting multi-node demo..."
            start_node 8080 "Node A" false
            sleep 2
            start_node 8081 "Node B" false
            sleep 2
            
            echo "✅ Two nodes started in background"
            echo "   Node A: Port 8080"
            echo "   Node B: Port 8081"
            echo ""
            echo "Press Enter to stop nodes..."
            read
            
            stop_nodes
            ;;
        3)
            echo "🔵 Testing Bluetooth functionality..."
            echo "   Note: This requires Bluetooth hardware and proper permissions"
            cargo run --release -- start --port 8080 --verbose
            ;;
        4)
            echo "👋 Goodbye!"
            exit 0
            ;;
        *)
            echo "❌ Invalid choice"
            exit 1
            ;;
    esac
}

# Cleanup function
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    stop_nodes
    rm -rf demo_data
}

# Set up cleanup trap
trap cleanup EXIT

# Run the demo
run_demo
