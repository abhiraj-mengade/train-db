//! Basic usage test for the lightweight RPC node

use anyhow::Result;
use lightweight_rpc_node::{Config, LightweightRpcNode, utils};

#[tokio::main]
async fn main() -> Result<()> {
    utils::setup_logging("info");

    println!("🚀 Starting Lightweight RPC Node Example");

    let mut config = Config::default();
    config.node.data_dir = "./example-data".into();
    config.rpc.listen_addr = "127.0.0.1:8547".parse()?;

    utils::ensure_directory(&config.node.data_dir)?;
    utils::validate_config(&config)?;

    let mut node = LightweightRpcNode::new(config).await?;
    node.start().await?;

    println!("✅ Node started on http://127.0.0.1:8547");
    println!("Press Ctrl+C to stop...");

    tokio::signal::ctrl_c().await?;

    println!("🛑 Shutting down...");
    node.stop().await?;
    println!("✅ Node stopped successfully");

    Ok(())
}
