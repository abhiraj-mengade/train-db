# Lightweight RPC Node

A high-performance Ethereum RPC node with multi-transport synchronization using train-db SDK concepts.

## 🚀 Features

- **Multi-Transport Sync**: HTTP primary with Bluetooth mesh fallback
- **train-db Integration**: P2P mesh networking for hostile environments
- **EVM Compatible**: Works with Foundry/Anvil and other EVM networks
- **Persistent Storage**: RocksDB-based blockchain state storage
- **JSON-RPC API**: Compatible with standard Ethereum clients

## 📦 Quick Start

```bash
# Build the project
cargo build --release --features bluetooth

# Run with default configuration
./target/release/rpc-node

# Generate configuration file
./target/release/rpc-node generate-config

# Run with custom config
./target/release/rpc-node --config config.toml
```

## 🔧 Configuration

The node connects to a BlockTrain devnet by default:

- **RPC URL**: http://blocktrain.local:8545
- **Chain ID**: 31337
- **Listen Address**: 127.0.0.1:8546

## 🌐 Architecture

```
Primary: HTTP Direct (to blocktrain.local:8545)
    ↓ (on failure)
Fallback: Bluetooth Mesh (train-db)
```

## 🛠️ train-db Integration Fixes

This implementation fixes several critical issues found in the train-db integration:

### Fixed Bluetooth Transport Issues:
- **Memory Leaks**: Proper cleanup of libp2p swarm and event loops
- **Connection Management**: Better peer discovery and connection handling
- **Message Validation**: Size limits and timestamp validation for P2P messages
- **Health Monitoring**: Connection quality tracking and automatic recovery

### Fixed State Management Issues:
- **Concurrency**: Using parking_lot::RwLock and DashMap for better thread safety
- **Batch Operations**: Optimized block updates for better performance
- **Cache Management**: Proper memory management with size limits
- **Data Validation**: Validation of synced state data from peers

### Fixed Transport Manager Issues:
- **Deadlock Prevention**: Improved locking strategy to prevent deadlocks
- **Health Scoring**: Transport health tracking with automatic failover
- **Error Recovery**: Better retry logic with exponential backoff
- **Connection Pooling**: HTTP client optimization for better performance

## 🔄 Multi-Transport Synchronization

### Primary Transport: HTTP
- Direct connection to blocktrain.local:8545
- Fast block/transaction synchronization
- Real-time state queries

### Fallback Transport: Bluetooth Mesh
- P2P gossip protocol using libp2p
- Bluetooth LE for device discovery
- State sharing between mesh nodes
- Survives network partitions

## 📊 API Endpoints

- `eth_blockNumber` - Get latest block number
- `eth_getBalance` - Get account balance
- `eth_getTransactionCount` - Get account nonce
- `net_version` - Get network/chain ID
- `web3_clientVersion` - Get client version
- `debug_syncStatus` - Get sync status and statistics

## 🧪 Testing

```bash
# Test basic functionality
curl -X POST http://127.0.0.1:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Check sync status
curl -X POST http://127.0.0.1:8546 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"debug_syncStatus","params":[],"id":1}'
```

## 📄 License

MIT License - Built for BlockTrain
