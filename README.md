# Train-DB: P2P Decentralized Key-Value Database
<img width="1344" height="768" alt="train-db" src="https://github.com/user-attachments/assets/b0deb0d8-b36d-4da7-b569-1d612356917e" />

A resilient, peer-to-peer key-value database designed for hostile network conditions, featuring multi-transport mesh networking with both Wi-Fi and Bluetooth support.

## ✨ Features

- **🔗 Multi-Transport Mesh**: Seamlessly combines Wi-Fi and Bluetooth connectivity
- **🌐 P2P Architecture**: Decentralized network with no single point of failure
- **💾 Persistent Storage**: RocksDB backend for reliable data persistence
- **📡 Real-time Gossiping**: Efficient data synchronization using GossipSub
- **🔵 Bluetooth Integration**: Continuous peer discovery and data sharing over Bluetooth
- **🌐 HTTP API**: RESTful API for easy integration
- **📊 Web Dashboard**: Real-time monitoring and statistics
- **⚡ Auto-Mesh Demo**: Intelligent transport switching and peer discovery
- **🦀 Rust Performance**: High-performance implementation in Rust

## Architecture

The system uses a "bridge node" concept where nodes can connect via multiple transports simultaneously:

- **Wi-Fi Mesh**: TCP connections over local network
- **Bluetooth Mesh**: Direct device-to-device connections
- **Unified Protocol**: Single application layer regardless of transport

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+
- Bluetooth adapter (for Bluetooth functionality)
- Network connectivity (for Wi-Fi mesh)

### Installation

```bash
git clone <repository-url>
cd train-db
cargo build --features bluetooth --release
```

### 🎯 Multi-Device Setup (Recommended)

**Device 1 (Bootstrap Node):**
```bash
cargo run --features bluetooth -- --db-path ./device1-data start-with-api --node-port 8082 --api-port 8085
```

**Device 2 (Connecting Node):**
```bash
cargo run --features bluetooth -- --db-path ./device2-data start-with-api --node-port 8083 --api-port 8086 --bootstrap "/ip4/[DEVICE1_IP]/tcp/8082"
```

### 📊 Web Dashboard

Access the real-time dashboard at:
- Device 1: http://localhost:8085
- Device 2: http://localhost:8086

### 🌐 HTTP API Usage

**Set data:**
```bash
curl -X POST http://localhost:8085/api/keys \
  -H "Content-Type: application/json" \
  -d '{"key": "train:passengers", "value": "150"}'
```

**Get data:**
```bash
curl http://localhost:8086/api/keys/train:passengers
```

**List all keys:**
```bash
curl http://localhost:8085/api/keys
```

### 🔵 Bluetooth Testing

**Test Bluetooth functionality:**
```bash
cargo run --features bluetooth -- test-bluetooth
```

**Run auto-mesh demo:**
```bash
cargo run --features bluetooth -- mesh-demo --duration 30
```

## 🔄 Real-time Data Gossiping

The system automatically gossips data changes across all connected peers:

**What you'll see in the logs:**
```
📤 Broadcasting data change: train:passengers = 150
📨 Received gossiped message from peer: [peer_id]
🔄 Gossiping: Setting key 'train:passengers' = '150'
✅ Successfully applied gossiped data: train:passengers = 150
```

**Transport detection:**
```
📡 TCP/Wi-Fi Connected to peer: [peer_id]
🔵 Bluetooth Connected to peer: [peer_id]
```

## 🌐 Network Protocols

### Transport Layer
- **TCP**: Primary transport for Wi-Fi mesh
- **Bluetooth LE**: Direct device connections via btleplug
- **mDNS**: Service discovery on local network
- **Bluetooth SDP**: Service discovery for Bluetooth

### Application Layer
- **GossipSub**: Message broadcasting and data sync
- **Identify**: Peer identification and metadata
- **Ping**: Connection health monitoring

### 🔵 Bluetooth Integration
- **Continuous Discovery**: Scans for peers every 10 seconds
- **Automatic Connection**: Connects to discovered Bluetooth peers
- **Transport-Agnostic**: Data gossiping works over any transport
- **Fallback Support**: Seamlessly switches to Bluetooth when Wi-Fi fails

## Configuration

### Environment Variables

```bash
# Enable debug logging
export RUST_LOG=debug

# Set database path
export TRAIN_DB_PATH=/path/to/database
```

### CLI Options

```bash
train-db [OPTIONS] <COMMAND>

Commands:
  start              Start the P2P node
  set                Set a key-value pair
  get                Get a value by key
  delete             Delete a key
  list               List all keys
  info               Show node information
  api                Start HTTP API server
  test-bluetooth     Test Bluetooth functionality
  mesh-demo          Run auto-mesh demo
  start-with-api     Start P2P node with integrated API server

Options:
  -d, --db-path <PATH>    Path to the database directory [default: ./data]
  -v, --verbose          Enable verbose logging
  -h, --help             Print help
```

### 🎛️ Available Commands

**P2P Node Commands:**
- `start` - Start basic P2P node
- `start-with-api` - Start P2P node with integrated HTTP API
- `api` - Start standalone HTTP API server

**Data Commands:**
- `set <key> <value>` - Set a key-value pair
- `get <key>` - Get a value by key
- `delete <key>` - Delete a key
- `list` - List all keys
- `info` - Show node information

**Testing Commands:**
- `test-bluetooth` - Test Bluetooth functionality
- `mesh-demo --duration <seconds>` - Run auto-mesh demo

## Development

### Project Structure

```
src/
├── main.rs          # CLI interface and main entry point
├── network.rs       # P2P networking and libp2p integration
├── storage.rs       # RocksDB storage abstraction
├── bluetooth.rs     # Bluetooth transport implementation
├── simple_node.rs   # HTTP API server implementation
├── mesh_node.rs     # Auto-mesh demo implementation
├── dashboard.rs     # Web dashboard implementation
└── cli.rs          # Interactive command-line interface

dashboard/
└── index.html      # Web dashboard UI
```

### Building

```bash
# Debug build
cargo build --features bluetooth

# Release build
cargo build --features bluetooth --release

# Run tests
cargo test --features bluetooth

# Run with logging
RUST_LOG=debug cargo run --features bluetooth
```

### 🧪 Testing Multi-Transport

**Multi-Device Testing:**
1. **Start multiple nodes on different machines**
2. **Enable Bluetooth on all devices**
3. **Verify automatic peer discovery**
4. **Test data synchronization across transports**

**Bluetooth-Only Testing:**
1. **Turn off Wi-Fi on both devices**
2. **Keep Bluetooth enabled**
3. **Verify data gossiping continues over Bluetooth**
4. **Check logs for Bluetooth connection indicators**

**Demo Scripts:**
```bash
# Run complete demo
./demo_complete.sh

# Test P2P gossiping
./test_p2p_with_data.sh

# Test Bluetooth functionality
./demo_bluetooth.sh

# Test auto-mesh
./demo_mesh.sh
```

## 🎯 Use Cases

- **🚂 Train/Plane Networks**: Reliable connectivity in moving vehicles
- **🚨 Emergency Communications**: Mesh networking in disaster scenarios
- **🏭 IoT Clusters**: Device-to-device communication without internet
- **📱 Offline Collaboration**: Local data sharing without central servers
- **🎮 Gaming Networks**: Low-latency multiplayer without servers
- **🏢 Office Networks**: Secure local data sharing
- **🌍 Remote Areas**: Connectivity without internet infrastructure

## 🔧 Advanced Features

### 📊 Web Dashboard
- **Real-time Statistics**: Peer connections, message counts, uptime
- **Database Browser**: View all key-value pairs
- **Network Monitor**: Track transport usage and connection health
- **Activity Log**: See all gossiping activity in real-time

### 🔵 Bluetooth Integration
- **Continuous Discovery**: Automatically finds nearby peers
- **Transport Switching**: Seamlessly switches between Wi-Fi and Bluetooth
- **Fallback Support**: Maintains connectivity when primary transport fails
- **Cross-Platform**: Works on macOS, Linux, and Windows

### 🌐 HTTP API
- **RESTful Interface**: Easy integration with other applications
- **JSON Support**: Standard data format for all operations
- **CORS Enabled**: Works with web applications
- **Real-time Updates**: Changes are immediately gossiped to all peers

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📄 License

MIT License - see LICENSE file for details

<img width="1285" height="682" alt="Screenshot 2025-09-22 at 4 33 06 AM" src="https://github.com/user-attachments/assets/aef7d76f-9fec-453c-91ae-d32195c7fa2b" />

<img width="745" height="422" alt="Screenshot 2025-09-22 at 4 58 44 AM" src="https://github.com/user-attachments/assets/5c52112b-f7be-4d21-8d36-1e5717fb9373" />

<img width="745" height="357" alt="Screenshot 2025-09-22 at 4 58 52 AM" src="https://github.com/user-attachments/assets/a48cf2da-fee6-40e9-8961-027251a7c343" />

---


**Made with ❤️ for BlockTrain**

*Train-DB: Resilient P2P networking for hostile environments*

