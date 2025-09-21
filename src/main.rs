use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, error, warn};
use tracing_subscriber;
use libp2p::Multiaddr;

mod network;  // Re-enabled - let's test if it works now
mod storage;
// mod cli;  // Temporarily disabled
mod bluetooth;
mod simple_node;
mod mesh_node;

use network::TrainDBNode;
use storage::{RocksDBStorage, Storage};
// use cli::InteractiveCLI;
use simple_node::SimpleTrainDBNode;

#[derive(Parser)]
#[command(name = "train-db")]
#[command(about = "P2P Decentralized Key-Value DB for hostile Network Conditions")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Path to the database directory
    #[arg(short, long, default_value = "./data")]
    db_path: PathBuf,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the P2P node
    Start {
        /// Port to listen on (0 for random)
        #[arg(short, long, default_value = "0")]
        port: u16,
        
        /// Bootstrap peer addresses (multiaddr format)
        #[arg(short, long)]
        bootstrap: Vec<String>,
        
        /// Enable interactive CLI mode
        #[arg(short, long)]
        interactive: bool,
    },
    /// Set a key-value pair
    Set {
        key: String,
        value: String,
    },
    /// Get a value by key
    Get {
        key: String,
    },
    /// Delete a key
    Delete {
        key: String,
    },
    /// List all keys
    List,
    /// Show node information
    Info,
    /// Start HTTP API server
    Api {
        /// Port for HTTP API server
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Test Bluetooth functionality
    TestBluetooth,
    /// Run auto-mesh demo
    MeshDemo {
        /// Duration of demo in seconds
        #[arg(short, long, default_value = "30")]
        duration: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("train_db={},libp2p=info", log_level))
        .init();
    
    info!("Starting Train-DB P2P Node");
    
    // Create database directory if it doesn't exist
    std::fs::create_dir_all(&cli.db_path)?;
    
    match cli.command {
        Commands::Start { port, bootstrap, interactive } => {
            start_node(cli.db_path, port, bootstrap, interactive).await?;
        }
        Commands::Set { key, value } => {
            set_key(cli.db_path, key, value).await?;
        }
        Commands::Get { key } => {
            get_key(cli.db_path, key).await?;
        }
        Commands::Delete { key } => {
            delete_key(cli.db_path, key).await?;
        }
        Commands::List => {
            list_keys(cli.db_path).await?;
        }
        Commands::Info => {
            show_info(cli.db_path).await?;
        }
        Commands::Api { port } => {
            start_api_server(cli.db_path, port).await?;
        }
        Commands::TestBluetooth => {
            test_bluetooth().await?;
        }
        Commands::MeshDemo { duration } => {
            run_mesh_demo(cli.db_path, duration).await?;
        }
    }
    
    Ok(())
}

async fn start_node(db_path: PathBuf, port: u16, bootstrap: Vec<String>, interactive: bool) -> Result<()> {
    info!("Starting P2P node on port {}", port);
    
    let storage = RocksDBStorage::new(&db_path)?;
    let mut node = TrainDBNode::new(storage, port).await?;
    
    // Connect to bootstrap peers if provided
    for bootstrap_addr in bootstrap {
        info!("Connecting to bootstrap peer: {}", bootstrap_addr);
        if let Ok(addr) = bootstrap_addr.parse::<Multiaddr>() {
            node.dial(addr).await?;
        }
    }
    
    if interactive {
        info!("Starting interactive CLI mode");
        // TODO: Implement interactive CLI
        println!("Interactive CLI not yet implemented. Use 'api' command for HTTP interface.");
    }
    
    // Run the node
    node.run().await?;
    
    Ok(())
}

async fn set_key(db_path: PathBuf, key: String, value: String) -> Result<()> {
    let storage = RocksDBStorage::new(db_path)?;
    storage.set(&key, &value)?;
    println!("Set {} = {}", key, value);
    Ok(())
}

async fn get_key(db_path: PathBuf, key: String) -> Result<()> {
    let storage = RocksDBStorage::new(db_path)?;
    match storage.get(&key)? {
        Some(value) => println!("{} = {}", key, value),
        None => println!("Key '{}' not found", key),
    }
    Ok(())
}

async fn delete_key(db_path: PathBuf, key: String) -> Result<()> {
    let storage = RocksDBStorage::new(db_path)?;
    storage.delete(&key)?;
    println!("Deleted key: {}", key);
    Ok(())
}

async fn list_keys(db_path: PathBuf) -> Result<()> {
    let storage = RocksDBStorage::new(db_path)?;
    let keys = storage.list_keys()?;
    if keys.is_empty() {
        println!("No keys found");
    } else {
        println!("Keys:");
        for key in keys {
            println!("  {}", key);
        }
    }
    Ok(())
}

async fn show_info(db_path: PathBuf) -> Result<()> {
    let storage = RocksDBStorage::new(&db_path)?;
    let info = storage.info()?;
    println!("Database Info:");
    println!("  Path: {:?}", db_path);
    println!("  Keys: {}", info.key_count);
    println!("  Size: {} bytes", info.size_bytes);
    Ok(())
}

async fn start_api_server(db_path: PathBuf, port: u16) -> Result<()> {
    info!("Starting TrainDB HTTP API server on port {}", port);
    
    let storage = RocksDBStorage::new(&db_path)?;
    let node = SimpleTrainDBNode::new(Box::new(storage), port);
    
    node.run().await?;
    
    Ok(())
}

async fn test_bluetooth() -> Result<()> {
    info!("Testing Bluetooth functionality...");
    
    #[cfg(feature = "bluetooth")]
    {
        use crate::bluetooth::{BluetoothTransport, BluetoothDiscovery, BluetoothConnectionManager, BluetoothMessageHandler, MultiTransport};
        
        info!("Bluetooth feature is enabled!");
        
        // Test Bluetooth transport
        info!("Creating Bluetooth transport...");
        let mut bt_transport = BluetoothTransport::new()?;
        info!("Bluetooth transport created successfully");
        
        // Test starting Bluetooth
        info!("Starting Bluetooth transport...");
        bt_transport.start().await?;
        info!("Bluetooth transport started successfully");
        
        // Test discovery
        info!("Testing Bluetooth discovery...");
        let mut discovery = BluetoothDiscovery::new()?;
        discovery.start_discovery().await?;
        info!("Bluetooth discovery started");
        
        // Test connection manager
        info!("Testing Bluetooth connection manager...");
        let mut conn_manager = BluetoothConnectionManager::new()?;
        info!("Bluetooth connection manager created");
        
        // Test message handler
        info!("Testing Bluetooth message handler...");
        let msg_handler = BluetoothMessageHandler::new();
        info!("Bluetooth message handler created");
        
        // Discover peers
        info!("Discovering Bluetooth peers...");
        let peers = bt_transport.discover_peers().await?;
        info!("Discovered {} Bluetooth peers", peers.len());
        
        for peer in &peers {
            info!("Found peer: {}", peer);
        }
        
        // Test sending a message
        if !peers.is_empty() {
            let test_peer = peers[0];
            info!("Testing message sending to peer: {}", test_peer);
            let test_message = b"Hello from TrainDB!".to_vec();
            bt_transport.send_message(test_peer, test_message).await?;
            info!("Message sent successfully");
        }
        
        // Stop Bluetooth
        info!("Stopping Bluetooth transport...");
        bt_transport.stop().await?;
        info!("Bluetooth transport stopped");
        
        info!("✅ Bluetooth functionality test completed successfully!");
    }
    
    #[cfg(not(feature = "bluetooth"))]
    {
        warn!("Bluetooth feature is not enabled. Build with --features bluetooth to test Bluetooth functionality.");
        info!("To enable Bluetooth support:");
        info!("  cargo build --features bluetooth");
        info!("  cargo run --features bluetooth test-bluetooth");
    }
    
    Ok(())
}

async fn run_mesh_demo(_db_path: PathBuf, duration: u64) -> Result<()> {
    info!("🚀 Starting Auto-Mesh Demo");
    
    #[cfg(feature = "bluetooth")]
    {
        use crate::mesh_node::AutoMeshNode;
        use tempfile::TempDir;
        
        // Use a temporary database for the demo to avoid lock conflicts
        let temp_dir = TempDir::new()?;
        let temp_db_path = temp_dir.path().join("mesh_demo_db");
        
        let storage = RocksDBStorage::new(&temp_db_path)?;
        let mut mesh_node = AutoMeshNode::new(storage)?;
        
        mesh_node.start().await?;
        mesh_node.run_mesh_demo(duration).await?;
        
        info!("✅ Auto-Mesh Demo completed successfully!");
    }
    
    #[cfg(not(feature = "bluetooth"))]
    {
        warn!("Bluetooth feature is not enabled. Build with --features bluetooth to run the mesh demo.");
        info!("To enable Bluetooth support:");
        info!("  cargo build --features bluetooth");
        info!("  cargo run --features bluetooth mesh-demo");
    }
    
    Ok(())
}
