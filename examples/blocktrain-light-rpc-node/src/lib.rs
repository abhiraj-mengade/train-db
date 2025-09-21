//! # Lightweight RPC Node
//! 
//! A lightweight Ethereum RPC node with multi-transport synchronization capabilities.

pub mod config;
pub mod blockchain_state;
pub mod transport;
pub mod sync_engine;
pub mod rpc_server;
pub mod utils;

use std::sync::Arc;
use anyhow::Result;
use parking_lot::RwLock;

pub use config::Config;
use blockchain_state::StateManager;
use sync_engine::SyncEngine;
use rpc_server::RpcServer;
use transport::{TransportManager, http_transport::HttpTransport};

#[cfg(feature = "bluetooth")]
use transport::bluetooth_transport::BluetoothTransport;

/// Main lightweight RPC node structure
pub struct LightweightRpcNode {
    config: Config,
    state_manager: Arc<StateManager>,
    sync_engine: Arc<SyncEngine>,
    rpc_server: RpcServer,
    is_running: bool,
}

impl LightweightRpcNode {
    pub async fn new(config: Config) -> Result<Self> {
        utils::ensure_directory(&config.node.data_dir)?;
        utils::ensure_directory(config.storage.db_path.parent().unwrap_or(&config.node.data_dir))?;

        let state_manager = Arc::new(StateManager::new(
            &config.storage.db_path,
            config.blockchain.chain_id,
        )?);

        let transport_manager = Self::create_transport_manager(&config, state_manager.clone()).await?;
        let transport_manager = Arc::new(RwLock::new(transport_manager));

        let sync_engine = Arc::new(SyncEngine::new(
            config.clone(),
            state_manager.clone(),
            transport_manager.clone(),
        ));

        let rpc_server = RpcServer::new(
            config.clone(),
            state_manager.clone(),
            sync_engine.clone(),
            transport_manager,
        );

        Ok(Self {
            config,
            state_manager,
            sync_engine,
            rpc_server,
            is_running: false,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        log::info!("Starting Lightweight RPC Node");
        self.sync_engine.start().await?;
        self.rpc_server.start().await?;

        self.is_running = true;
        log::info!("Lightweight RPC Node started successfully");

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }

        log::info!("Stopping Lightweight RPC Node");
        self.rpc_server.stop();
        self.sync_engine.stop().await?;

        self.is_running = false;
        log::info!("Lightweight RPC Node stopped");

        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn get_sync_status(&self) -> blockchain_state::SyncStatus {
        self.sync_engine.get_sync_status()
    }

    pub async fn get_latest_block_number(&self) -> u64 {
        self.state_manager.get_latest_block_number().await
    }

    pub async fn force_sync(&self, from_block: Option<u64>, to_block: Option<u64>) -> Result<()> {
        self.sync_engine.force_sync(from_block, to_block).await
    }

    async fn create_transport_manager(
        config: &Config,
        state_manager: Arc<StateManager>,
    ) -> Result<TransportManager> {
        let http_transport = Box::new(HttpTransport::new(
            config.blockchain.primary_rpc_url.clone(),
            config.blockchain.chain_id,
            config.transport.timeout_ms,
        ));

        #[cfg(feature = "bluetooth")]
        let fallback_transport = {
            let node_id = config.node.node_id
                .clone()
                .unwrap_or_else(|| utils::generate_node_id());

            let bluetooth_transport = BluetoothTransport::new(node_id, state_manager).await?;
            Some(Box::new(bluetooth_transport) as Box<dyn transport::Transport>)
        };

        #[cfg(not(feature = "bluetooth"))]
        let fallback_transport = None;

        Ok(TransportManager::new(http_transport, fallback_transport))
    }
}

pub async fn run_with_config(config: Config) -> Result<()> {
    let mut node = LightweightRpcNode::new(config).await?;
    node.start().await?;

    match tokio::signal::ctrl_c().await {
        Ok(()) => log::info!("Received shutdown signal"),
        Err(err) => log::error!("Unable to listen for shutdown signal: {}", err),
    }

    node.stop().await?;
    Ok(())
}
