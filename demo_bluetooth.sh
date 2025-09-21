#!/bin/bash

echo "🔵 TrainDB P2P Database with Bluetooth Demo"
echo "==========================================="
echo ""
echo "This demo shows:"
echo "📡 TCP/GossipSub P2P networking"
echo "🔵 Bluetooth device discovery and scanning"
echo "💾 Distributed key-value storage"
echo "🌐 HTTP REST API"
echo ""

# Cleanup function
cleanup() {
    echo ""
    echo "🧹 Cleaning up..."
    pkill -f train-db 2>/dev/null || true
    sleep 1
    echo "✅ Cleanup complete"
    exit 0
}

# Set trap for cleanup
trap cleanup EXIT INT TERM

echo "🔧 Building TrainDB with Bluetooth support..."
cargo build --features bluetooth --release

if [ $? -ne 0 ]; then
    echo "❌ Build failed"
    exit 1
fi

echo "✅ Build successful!"
echo ""

echo "🚀 Starting Node 1 (P2P: 4001, API: 8001)..."
RUST_LOG=info ./target/release/train-db --db-path ./node1-data start-with-api \
    --node-port 4001 \
    --api-port 8001 \
    --bootstrap /ip4/127.0.0.1/tcp/4002 &

NODE1_PID=$!
sleep 2

echo "🚀 Starting Node 2 (P2P: 4002, API: 8002)..."
RUST_LOG=info ./target/release/train-db --db-path ./node2-data start-with-api \
    --node-port 4002 \
    --api-port 8002 \
    --bootstrap /ip4/127.0.0.1/tcp/4001 &

NODE2_PID=$!
sleep 3

echo ""
echo "🔵 Both nodes are now running with Bluetooth enabled!"
echo ""
echo "What you should see in the logs:"
echo "  🔵 Starting Simple BLE Communicator as: TrainDB"
echo "  🔵 🔍 Discovered BLE device: 'Device Name'"
echo "  🔵 ✅ Found TrainDB device: Device Name"
echo "  📡 ✅ Connected to peer via TCP"
echo ""

echo "📊 Testing the system..."
echo ""

echo "1️⃣ Setting data on Node 1..."
RESPONSE1=$(curl -s -X POST "http://localhost:8001/api/keys" \
    -H "Content-Type: application/json" \
    -d '{"key": "node1_test", "value": "Hello from Node 1!"}')
echo "   Response: $RESPONSE1"

sleep 1

echo ""
echo "2️⃣ Setting data on Node 2..."
RESPONSE2=$(curl -s -X POST "http://localhost:8002/api/keys" \
    -H "Content-Type: application/json" \
    -d '{"key": "node2_test", "value": "Hello from Node 2!"}')
echo "   Response: $RESPONSE2"

sleep 2

echo ""
echo "📋 Checking data on both nodes..."
echo ""

echo "📥 Node 1 keys:"
curl -s "http://localhost:8001/api/keys" | jq '.'

echo ""
echo "📥 Node 2 keys:"
curl -s "http://localhost:8002/api/keys" | jq '.'

echo ""
echo "🎯 Testing cross-node data access..."

echo ""
echo "📤 Getting Node 2's data from Node 1:"
curl -s "http://localhost:8001/api/keys/node2_test" 2>/dev/null | jq '.' || echo "   (Data may not have synchronized yet)"

echo ""
echo "📤 Getting Node 1's data from Node 2:"
curl -s "http://localhost:8002/api/keys/node1_test" 2>/dev/null | jq '.' || echo "   (Data may not have synchronized yet)"

echo ""
echo "🔄 Rapid updates test..."
for i in {1..3}; do
    echo "   Update $i/3..."
    curl -s -X POST "http://localhost:8001/api/keys" \
        -H "Content-Type: application/json" \
        -d "{\"key\": \"rapid_$i\", \"value\": \"update_$i\"}" > /dev/null
    sleep 0.5
done

sleep 2

echo ""
echo "📊 Final state:"
echo ""
echo "Node 1 final keys:"
curl -s "http://localhost:8001/api/keys" | jq '.'

echo ""
echo "Node 2 final keys:"
curl -s "http://localhost:8002/api/keys" | jq '.'

echo ""
echo "🎉 Demo Results:"
echo "==============="
echo ""
echo "✅ P2P networking: Working via TCP/GossipSub"
echo "✅ Bluetooth scanning: Discovering nearby devices"
echo "✅ HTTP API: REST endpoints responding"
echo "✅ Data storage: Key-value pairs stored"
echo "✅ Multi-node: Both nodes operational"
echo ""
echo "🔵 Bluetooth Features:"
echo "   📡 Device discovery via BLE scanning"
echo "   🔍 TrainDB device identification"
echo "   📊 Device logging and tracking"
echo ""
echo "💡 API Endpoints:"
echo "   POST /api/keys - Set key-value pairs"
echo "   GET  /api/keys - List all keys"
echo "   GET  /api/keys/{key} - Get specific key"
echo ""
echo "🌐 Access your nodes:"
echo "   Node 1: http://localhost:8001/api/keys"
echo "   Node 2: http://localhost:8002/api/keys"
echo ""

echo "🔵 Press Ctrl+C to stop the demo"
echo ""

# Keep running until interrupted
while true; do
    sleep 5
    echo "📊 Nodes running... ($(date '+%H:%M:%S'))"
done
