#!/bin/bash

# Multi-Device Train-DB Testing Script
# This script helps coordinate testing across multiple devices

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

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

echo "🚂 Train-DB Multi-Device Testing"
echo "================================"
echo ""

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    print_error "Please run this script from the train-db project root directory"
    exit 1
fi

print_status "This script will help you test Train-DB across multiple devices"
echo ""

# Function to show device setup instructions
show_device_setup() {
    print_demo "📱 Device Setup Instructions"
    echo "=============================="
    echo ""
    print_mesh "On EACH device you want to test:"
    print_mesh "1. Clone this repository:"
    print_mesh "   git clone <your-repo-url>"
    print_mesh "   cd train-db"
    echo ""
    print_mesh "2. Build with Bluetooth support:"
    print_mesh "   cargo build --features bluetooth"
    echo ""
    print_mesh "3. Ensure Bluetooth is enabled"
    print_mesh "4. Connect to the same Wi-Fi network (for initial testing)"
    echo ""
}

# Function to show testing scenarios
show_testing_scenarios() {
    print_demo "🎯 Testing Scenarios"
    echo "===================="
    echo ""
    print_success "Scenario 1: Basic Multi-Device Mesh"
    print_mesh "  • Start mesh demo on all devices simultaneously"
    print_mesh "  • Watch for peer discovery over Wi-Fi and Bluetooth"
    print_mesh "  • Verify mesh quality metrics"
    echo ""
    print_success "Scenario 2: Transport Fallback Testing"
    print_mesh "  • Start all devices on Wi-Fi"
    print_mesh "  • Disconnect Wi-Fi on one device"
    print_mesh "  • Observe automatic switch to Bluetooth"
    echo ""
    print_success "Scenario 3: Data Synchronization"
    print_mesh "  • Start API servers on different ports"
    print_mesh "  • Set keys on one device"
    print_mesh "  • Verify keys appear on other devices"
    echo ""
}

# Function to show commands for each device
show_device_commands() {
    print_demo "🖥️  Commands for Each Device"
    echo "============================="
    echo ""
    print_status "Device 1 (Primary - Port 8080):"
    echo "  cargo run api --port 8080"
    echo "  # OR"
    echo "  cargo run --features bluetooth mesh-demo --duration 60"
    echo ""
    print_status "Device 2 (Secondary - Port 8081):"
    echo "  cargo run api --port 8081"
    echo "  # OR"
    echo "  cargo run --features bluetooth mesh-demo --duration 60"
    echo ""
    print_status "Device 3 (Optional - Port 8082):"
    echo "  cargo run api --port 8082"
    echo "  # OR"
    echo "  cargo run --features bluetooth mesh-demo --duration 60"
    echo ""
}

# Function to show testing commands
show_testing_commands() {
    print_demo "🧪 Testing Commands"
    echo "===================="
    echo ""
    print_status "Test data synchronization:"
    echo "  # On Device 1:"
    echo "  curl -X POST http://localhost:8080/api/keys/train-status -d 'On time'"
    echo ""
    echo "  # On Device 2:"
    echo "  curl http://localhost:8081/api/keys/train-status"
    echo ""
    print_status "Monitor mesh status:"
    echo "  curl http://localhost:8080/api/info"
    echo "  curl http://localhost:8081/api/info"
    echo ""
    print_status "Test Bluetooth discovery:"
    echo "  cargo run --features bluetooth test-bluetooth"
    echo ""
}

# Function to show what to look for
show_expected_results() {
    print_demo "📊 Expected Results"
    echo "==================="
    echo ""
    print_success "✅ Successful Multi-Device Mesh:"
    print_mesh "  • Peer discovery messages in logs"
    print_mesh "  • Transport health monitoring"
    print_mesh "  • Mesh quality score > 0.7"
    print_mesh "  • Automatic transport switching"
    echo ""
    print_success "✅ Sample Log Output:"
    print_mesh "  📡 Found 2 peers over Wi-Fi"
    print_mesh "  🔵 Found 1 peers over Bluetooth"
    print_mesh "  📊 Mesh Status: Total Peers: 3, Quality: 0.85/1.0"
    print_mesh "  ✅ Excellent mesh connectivity!"
    echo ""
}

# Function to show troubleshooting
show_troubleshooting() {
    print_demo "🔧 Troubleshooting"
    echo "==================="
    echo ""
    print_warning "Common Issues:"
    print_mesh "  • Bluetooth not enabled: Check system settings"
    print_mesh "  • No peers found: Ensure devices are in range"
    print_mesh "  • Build errors: Run 'cargo clean && cargo build --features bluetooth'"
    print_mesh "  • Port conflicts: Use different ports for each device"
    echo ""
    print_status "Debug mode:"
    echo "  RUST_LOG=debug cargo run --features bluetooth mesh-demo --duration 30"
    echo ""
}

# Main menu
show_menu() {
    echo ""
    print_demo "Choose a testing option:"
    echo "1. Show device setup instructions"
    echo "2. Show testing scenarios"
    echo "3. Show commands for each device"
    echo "4. Show testing commands"
    echo "5. Show expected results"
    echo "6. Show troubleshooting"
    echo "7. Run local mesh demo (single device)"
    echo "8. Exit"
    echo ""
    read -p "Enter your choice (1-8): " choice
    
    case $choice in
        1) show_device_setup ;;
        2) show_testing_scenarios ;;
        3) show_device_commands ;;
        4) show_testing_commands ;;
        5) show_expected_results ;;
        6) show_troubleshooting ;;
        7) run_local_demo ;;
        8) exit 0 ;;
        *) print_error "Invalid choice. Please enter 1-8." ;;
    esac
}

# Function to run local demo
run_local_demo() {
    print_demo "🚀 Running Local Mesh Demo"
    echo "============================"
    echo ""
    print_status "This will run a 30-second mesh demo on this device"
    print_status "Use this to verify everything works before multi-device testing"
    echo ""
    read -p "Continue? (y/n): " confirm
    
    if [[ $confirm == "y" || $confirm == "Y" ]]; then
        print_status "Building and running mesh demo..."
        if cargo run --features bluetooth mesh-demo --duration 30; then
            print_success "✅ Local demo completed successfully!"
            print_mesh "Your device is ready for multi-device testing!"
        else
            print_error "❌ Local demo failed. Check the errors above."
        fi
    fi
}

# Check if Bluetooth is available
check_bluetooth() {
    print_status "Checking Bluetooth availability..."
    
    if [[ "$OSTYPE" == "darwin"* ]]; then
        if system_profiler SPBluetoothDataType | grep -q "Bluetooth Power: On"; then
            print_success "✅ Bluetooth is enabled on macOS"
        else
            print_warning "⚠️  Bluetooth may not be enabled. Enable it in System Preferences."
        fi
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if command -v bluetoothctl &> /dev/null; then
            print_success "✅ Bluetooth tools available on Linux"
        else
            print_warning "⚠️  Bluetooth tools not found. Install bluez package."
        fi
    else
        print_warning "⚠️  Bluetooth status check not supported on this OS"
    fi
}

# Main execution
main() {
    print_status "Checking system requirements..."
    
    # Check if Rust is installed
    if command -v cargo &> /dev/null; then
        print_success "✅ Rust/Cargo is installed"
    else
        print_error "❌ Rust/Cargo not found. Please install Rust first."
        exit 1
    fi
    
    # Check if project builds
    print_status "Building project..."
    if cargo build --features bluetooth --quiet; then
        print_success "✅ Project builds successfully"
    else
        print_error "❌ Build failed. Please fix build errors first."
        exit 1
    fi
    
    # Check Bluetooth
    check_bluetooth
    
    echo ""
    print_success "🎉 System is ready for multi-device testing!"
    echo ""
    
    # Show main menu
    while true; do
        show_menu
        echo ""
        read -p "Press Enter to continue or 'q' to quit: " continue_choice
        if [[ $continue_choice == "q" || $continue_choice == "Q" ]]; then
            break
        fi
    done
    
    print_success "🚂 Happy testing! Your Train-DB mesh is ready to go!"
}

# Run main function
main
