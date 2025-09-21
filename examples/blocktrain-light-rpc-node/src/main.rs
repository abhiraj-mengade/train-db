use clap::{Parser, Subcommand};
use anyhow::Result;
use std::path::PathBuf;
use lightweight_rpc_node::{Config, LightweightRpcNode, utils};

#[derive(Parser)]
#[command(name = "lightweight-rpc-node")]
#[command(about = "A lightweight Ethereum RPC node with multi-transport sync")]
#[command(version)]
struct Cli {
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[arg(short, long, default_value = "info")]
    log_level: String,

    #[arg(short, long)]
    data_dir: Option<PathBuf>,

    #[arg(long, default_value = "127.0.0.1:8546")]
    rpc_addr: String,

    #[arg(long, default_value = "http://blocktrain.local:8545")]
    primary_rpc: String,

    #[arg(long, default_value = "31337")]
    chain_id: u64,

    #[arg(long)]
    no_bluetooth: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Start,
    CheckConfig,
    GenerateConfig {
        #[arg(short, long, default_value = "config.toml")]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    utils::setup_logging(&cli.log_level);
    log::info!("Lightweight RPC Node v{}", env!("CARGO_PKG_VERSION"));

    match cli.command.unwrap_or(Commands::Start) {
        Commands::Start => start_node(cli).await,
        Commands::CheckConfig => check_config(cli).await,
        Commands::GenerateConfig { output } => generate_config(output).await,
    }
}

async fn start_node(cli: Cli) -> Result<()> {
    let config = load_config(cli).await?;
    utils::validate_config(&config)?;
    utils::ensure_directory(&config.node.data_dir)?;

    let mut node = LightweightRpcNode::new(config).await?;

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                log::info!("Received shutdown signal");
                let _ = shutdown_tx.send(());
            }
            Err(err) => {
                log::error!("Unable to listen for shutdown signal: {}", err);
            }
        }
    });

    node.start().await?;
    log::info!("Node started successfully! Press Ctrl+C to stop.");

    let _ = shutdown_rx.await;
    log::info!("Shutting down...");
    node.stop().await?;
    log::info!("Node stopped successfully");

    Ok(())
}

async fn load_config(cli: Cli) -> Result<Config> {
    let mut config = if let Some(config_path) = cli.config {
        Config::from_file(config_path.to_str().unwrap())?
    } else {
        Config::default()
    };

    if let Some(data_dir) = cli.data_dir {
        config.node.data_dir = data_dir;
    }

    config.blockchain.primary_rpc_url = cli.primary_rpc;
    config.blockchain.chain_id = cli.chain_id;
    config.rpc.listen_addr = cli.rpc_addr.parse()?;
    config.node.log_level = cli.log_level;

    Ok(config)
}

async fn check_config(cli: Cli) -> Result<()> {
    let config = load_config(cli).await?;

    match utils::validate_config(&config) {
        Ok(()) => {
            println!("✅ Configuration is valid");

            if utils::is_address_reachable(&format!("{}:8545", "blocktrain.local")).await {
                println!("✅ Primary RPC endpoint is reachable");
            } else {
                println!("⚠️  Primary RPC endpoint is not reachable");
            }

            Ok(())
        }
        Err(e) => {
            println!("❌ Configuration validation failed: {}", e);
            Err(e)
        }
    }
}

async fn generate_config(output_path: PathBuf) -> Result<()> {
    let config = Config::default();
    config.save_to_file(output_path.to_str().unwrap())?;
    println!("Generated configuration file: {}", output_path.display());
    Ok(())
}
