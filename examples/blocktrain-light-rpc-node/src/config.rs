use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use anyhow::{Result, Context, anyhow};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub node: NodeConfig,
    pub blockchain: BlockchainConfig,
    pub transport: TransportConfig,
    pub rpc: RpcConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeConfig {
    pub node_id: Option<String>,
    pub data_dir: PathBuf,
    pub log_level: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlockchainConfig {
    pub chain_id: u64,
    pub network_id: u64,
    pub primary_rpc_url: String,
    pub root_seed: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransportConfig {
    pub primary_transport: String,
    pub fallback_transport: String,
    pub sync_interval_ms: u64,
    pub max_retries: u32,
    pub timeout_ms: u64,
    pub max_peer_connections: u32,
    pub heartbeat_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RpcConfig {
    pub listen_addr: SocketAddr,
    pub cors_origins: Vec<String>,
    pub methods: Vec<String>,
    pub max_connections: u32,
    pub request_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub db_path: PathBuf,
    pub cache_size_mb: usize,
    pub sync_batch_size: usize,
    pub max_cache_age_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                node_id: None,
                data_dir: PathBuf::from("./data"),
                log_level: "info".to_string(),
            },
            blockchain: BlockchainConfig {
                chain_id: 31337,
                network_id: 31337,
                primary_rpc_url: "http://blocktrain.local:8545".to_string(),
                root_seed: "test test test test test test test test test test test junk".to_string(),
            },
            transport: TransportConfig {
                primary_transport: "http".to_string(),
                fallback_transport: "bluetooth".to_string(),
                sync_interval_ms: 5000,
                max_retries: 3,
                timeout_ms: 30000,
                max_peer_connections: 50,
                heartbeat_interval_ms: 60000,
            },
            rpc: RpcConfig {
                listen_addr: "127.0.0.1:8546".parse().unwrap(),
                cors_origins: vec!["*".to_string()],
                methods: vec![
                    "eth_blockNumber".to_string(),
                    "eth_getBalance".to_string(),
                    "eth_getTransactionCount".to_string(),
                    "eth_getBlockByNumber".to_string(),
                    "eth_getBlockByHash".to_string(),
                    "eth_sendRawTransaction".to_string(),
                    "eth_call".to_string(),
                    "eth_estimateGas".to_string(),
                    "eth_gasPrice".to_string(),
                    "eth_syncing".to_string(),
                    "net_version".to_string(),
                    "web3_clientVersion".to_string(),
                ],
                max_connections: 100,
                request_timeout_ms: 30000,
            },
            storage: StorageConfig {
                db_path: PathBuf::from("./data/blockchain.db"),
                cache_size_mb: 256,
                sync_batch_size: 100,
                max_cache_age_seconds: 3600,
            },
        }
    }
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path))?;
        config.validate()?;
        Ok(config)
    }

    pub fn save_to_file(&self, path: &str) -> Result<()> {
        self.validate()?;
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.blockchain.chain_id == 0 {
            return Err(anyhow!("Chain ID cannot be zero"));
        }

        if self.blockchain.primary_rpc_url.is_empty() {
            return Err(anyhow!("Primary RPC URL cannot be empty"));
        }

        if !self.blockchain.primary_rpc_url.starts_with("http://") && 
           !self.blockchain.primary_rpc_url.starts_with("https://") {
            return Err(anyhow!("Primary RPC URL must start with http:// or https://"));
        }

        if self.transport.timeout_ms == 0 {
            return Err(anyhow!("Timeout cannot be zero"));
        }

        if self.transport.sync_interval_ms == 0 {
            return Err(anyhow!("Sync interval cannot be zero"));
        }

        if self.rpc.listen_addr.port() == 0 {
            return Err(anyhow!("RPC listen port cannot be zero"));
        }

        if self.storage.cache_size_mb == 0 {
            return Err(anyhow!("Cache size cannot be zero"));
        }

        Ok(())
    }
}
