use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, error};
use tracing_subscriber;

// mod network;  // Temporarily disabled due to libp2p API issues
mod storage;
// mod cli;  // Temporarily disabled
mod bluetooth;
mod simple_node;

// use network::TrainDBNode;
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
            // start_node(cli.db_path, port, bootstrap, interactive).await?;
            println!("P2P node functionality temporarily disabled. Use 'api' command to start HTTP API server.");
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
    }
    
    Ok(())
}

// async fn start_node(db_path: PathBuf, port: u16, bootstrap: Vec<String>, interactive: bool) -> Result<()> {
//     // Temporarily disabled due to libp2p API issues
//     println!("P2P node functionality temporarily disabled. Use 'api' command to start HTTP API server.");
//     Ok(())
// }

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
