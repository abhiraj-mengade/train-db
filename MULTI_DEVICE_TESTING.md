# Multi-Device Train-DB Testing Guide

This guide explains how to test the Train-DB multi-transport mesh networking across multiple devices to demonstrate the intelligent transport switching behavior.

## 🎯 Testing Goals

- **Multi-Transport Mesh**: Test Wi-Fi and Bluetooth connectivity between devices
- **Intelligent Switching**: Demonstrate automatic fallback when Wi-Fi fails
- **Peer Discovery**: Show how devices find each other across different transports
- **Data Synchronization**: Verify key-value data syncs across the mesh
- **Fault Tolerance**: Test behavior when network conditions change

## 📱 Device Setup

### Minimum Requirements
- **2+ devices** (laptops, phones, tablets)
- **Rust installed** on each device
- **Bluetooth enabled** on all devices
- **Same Wi-Fi network** (for initial testing)

### Recommended Setup
- **Device 1**: Primary laptop (your current machine)
- **Device 2**: Secondary laptop or phone
- **Device 3**: Additional device (optional, for larger mesh)

## 🚀 Quick Start Testing

### Step 1: Clone and Build on All Devices

On each device, run:
```bash
# Clone the repository
git clone <your-repo-url>
cd train-db

# Build with Bluetooth support
cargo build --features bluetooth
```

### Step 2: Start Nodes on Each Device

**Device 1 (Primary)**:
```bash
# Start the mesh demo (this will be the "coordinator")
cargo run --features bluetooth mesh-demo --duration 60
```

**Device 2 (Secondary)**:
```bash
# Start another mesh demo
cargo run --features bluetooth mesh-demo --duration 60
```

**Device 3 (Optional)**:
```bash
# Start a third node
cargo run --features bluetooth mesh-demo --duration 60
```

## 🔬 Advanced Testing Scenarios

### Scenario 1: Wi-Fi Mesh Testing
1. Ensure all devices are on the same Wi-Fi network
2. Start nodes on all devices
3. Watch for peer discovery over Wi-Fi
4. Test data synchronization via HTTP API

### Scenario 2: Bluetooth Fallback Testing
1. Start nodes on all devices
2. Disconnect Wi-Fi on one device
3. Observe automatic switch to Bluetooth
4. Verify continued peer communication

### Scenario 3: Mixed Transport Testing
1. Have some devices on Wi-Fi, others on Bluetooth only
2. Start nodes and observe bridge behavior
3. Test message routing between different transport types

## 🛠️ Testing Commands

### Basic Mesh Demo
```bash
# Run 30-second demo
cargo run --features bluetooth mesh-demo --duration 30

# Run 2-minute demo
cargo run --features bluetooth mesh-demo --duration 120
```

### HTTP API Testing
```bash
# Start API server on device 1
cargo run api --port 8080

# Start API server on device 2 (different port)
cargo run api --port 8081

# Test data sync between devices
curl -X POST http://localhost:8080/api/keys/test-key -d "Hello from Device 1"
curl http://localhost:8081/api/keys/test-key
```

### Bluetooth-Only Testing
```bash
# Test Bluetooth functionality
cargo run --features bluetooth test-bluetooth
```

## 📊 What to Look For

### Successful Multi-Transport Mesh
- ✅ **Peer Discovery**: Devices find each other over Wi-Fi and Bluetooth
- ✅ **Transport Health**: Real-time monitoring shows both transports healthy
- ✅ **Mesh Quality**: Quality score > 0.7 indicates excellent connectivity
- ✅ **Automatic Switching**: Seamless fallback when Wi-Fi fails

### Expected Log Output
```
🚀 Starting Auto-Mesh Node
✅ Bluetooth transport started
🎯 Auto-Mesh Node is now running and discovering peers...
📡 Found 2 peers over Wi-Fi
🔵 Found 1 peers over Bluetooth
📊 Mesh Status:
   Total Peers: 3
   Wi-Fi Peers: 2 (healthy: true)
   Bluetooth Peers: 1 (healthy: true)
   Mesh Quality: 0.85/1.0
✅ Excellent mesh connectivity!
```

## 🚂 Train-Specific Testing

### Simulate Train Environment
1. **Start in Wi-Fi**: All devices connected to train Wi-Fi
2. **Simulate Tunnel**: Disconnect Wi-Fi on some devices
3. **Observe Fallback**: Watch automatic switch to Bluetooth mesh
4. **Simulate Recovery**: Reconnect Wi-Fi and observe restoration

### Real-World Scenarios
- **Moving between train cars**: Test connectivity as devices move
- **Intermittent Wi-Fi**: Simulate poor Wi-Fi coverage
- **Battery optimization**: Test behavior with low battery modes

## 🔧 Troubleshooting

### Common Issues

**Bluetooth Not Working**:
```bash
# Check Bluetooth status (macOS)
system_profiler SPBluetoothDataType

# Check Bluetooth status (Linux)
bluetoothctl show
```

**No Peers Found**:
- Ensure Bluetooth is enabled on all devices
- Check that devices are within Bluetooth range (~10 meters)
- Verify Wi-Fi network connectivity

**Build Errors**:
```bash
# Clean and rebuild
cargo clean
cargo build --features bluetooth
```

### Debug Mode
```bash
# Run with debug logging
RUST_LOG=debug cargo run --features bluetooth mesh-demo --duration 30
```

## 📈 Performance Testing

### Load Testing
```bash
# Test with multiple concurrent operations
for i in {1..10}; do
  curl -X POST http://localhost:8080/api/keys/key-$i -d "value-$i" &
done
```

### Stress Testing
- Start multiple nodes on same device (different ports)
- Test with 10+ simulated peers
- Monitor memory and CPU usage

## 🎮 Interactive Testing

### Manual Key-Value Testing
```bash
# Device 1: Set a key
curl -X POST http://localhost:8080/api/keys/train-status -d "On time"

# Device 2: Get the key (should sync via mesh)
curl http://localhost:8081/api/keys/train-status
```

### Real-Time Monitoring
```bash
# Watch mesh status in real-time
watch -n 2 'curl -s http://localhost:8080/api/info | jq'
```

## 🏆 Success Criteria

Your multi-device test is successful when:

1. **✅ Peer Discovery**: All devices find each other
2. **✅ Transport Switching**: Automatic fallback works
3. **✅ Data Sync**: Keys set on one device appear on others
4. **✅ Fault Tolerance**: Mesh continues working when transports fail
5. **✅ Recovery**: Mesh restores when transports come back online

## 🚀 Next Steps

Once basic multi-device testing works:

1. **Implement GossipSub**: Add real P2P message broadcasting
2. **Add libp2p Integration**: Connect the mesh to actual libp2p networking
3. **Performance Optimization**: Optimize for real-world train conditions
4. **Security**: Add encryption and authentication
5. **Mobile Apps**: Create mobile clients for passengers

## 📞 Support

If you encounter issues:
1. Check the logs for error messages
2. Verify all dependencies are installed
3. Ensure Bluetooth permissions are granted
4. Test with fewer devices first
5. Use debug logging to identify issues

Happy testing! 🚂✨
