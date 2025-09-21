# Train-DB: P2P Decentralized Key-Value Database
<img width="1344" height="768" alt="train-db" src="https://github.com/user-attachments/assets/b0deb0d8-b36d-4da7-b569-1d612356917e" />

A resilient, peer-to-peer key-value database designed for hostile network conditions, featuring multi-transport mesh networking with both Wi-Fi and Bluetooth support.

## Features

- **Multi-Transport Mesh**: Seamlessly combines Wi-Fi and Bluetooth connectivity
- **P2P Architecture**: Decentralized network with no single point of failure
- **Persistent Storage**: RocksDB backend for reliable data persistence
- **Gossip Protocol**: Efficient data synchronization using GossipSub
- **Interactive CLI**: Easy-to-use command-line interface
- **Rust Performance**: High-performance implementation in Rust

## Architecture

The system uses a "bridge node" concept where nodes can connect via multiple transports simultaneously:

- **Wi-Fi Mesh**: TCP connections over local network
- **Bluetooth Mesh**: Direct device-to-device connections
- **Unified Protocol**: Single application layer regardless of transport

## Quick Start

### Prerequisites

- Rust 1.70+
- Bluetooth adapter (for Bluetooth functionality)
- Network connectivity (for Wi-Fi mesh)

### Installation

```bash
git clone <repository-url>
cd train-db
cargo build --release
```

### Basic Usage

1. **Start a node with interactive CLI:**
```bash
cargo run -- start --interactive
```

2. **Start a node on specific port:**
```bash
cargo run -- start --port 8080
```

3. **Connect to bootstrap peers:**
```bash
cargo run -- start --bootstrap /ip4/192.168.1.100/tcp/8080/p2p/12D3KooW...
```

4. **Use CLI commands:**
```bash
# Set a key-value pair
set mykey "Hello, World!"

# Get a value
get mykey

# List all keys
list

# Delete a key
delete mykey
```

### Multi-Transport Demo

1. **Start Node A (Wi-Fi + Bluetooth):**
```bash
cargo run -- start --port 8080 --interactive
```

2. **Start Node B (Bluetooth only):**
```bash
cargo run -- start --port 8081 --interactive
```

3. **Node A will automatically discover Node B via Bluetooth**
4. **Data syncs across both transports seamlessly**

## Network Protocols

### Transport Layer
- **TCP**: Primary transport for Wi-Fi mesh
- **Bluetooth RFCOMM**: Direct device connections
- **mDNS**: Service discovery on local network
- **Bluetooth SDP**: Service discovery for Bluetooth

### Application Layer
- **GossipSub**: Message broadcasting and data sync
- **Identify**: Peer identification and metadata
- **Ping**: Connection health monitoring

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
  start     Start the P2P node
  set       Set a key-value pair
  get       Get a value by key
  delete    Delete a key
  list      List all keys
  info      Show node information

Options:
  -d, --db-path <PATH>    Path to the database directory [default: ./data]
  -v, --verbose          Enable verbose logging
  -h, --help             Print help
```

## Development

### Project Structure

```
src/
├── main.rs          # CLI interface and main entry point
├── network.rs       # P2P networking and libp2p integration
├── storage.rs       # RocksDB storage abstraction
├── bluetooth.rs     # Bluetooth transport implementation
└── cli.rs          # Interactive command-line interface
```

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

### Testing Multi-Transport

1. **Start multiple nodes on different machines**
2. **Enable Bluetooth on all devices**
3. **Verify automatic peer discovery**
4. **Test data synchronization across transports**

## Use Cases

- **Train/Plane Networks**: Reliable connectivity in moving vehicles
- **Emergency Communications**: Mesh networking in disaster scenarios
- **IoT Clusters**: Device-to-device communication without internet
- **Offline Collaboration**: Local data sharing without central servers

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT License - see LICENSE file for details

