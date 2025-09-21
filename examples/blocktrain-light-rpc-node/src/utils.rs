use anyhow::{Result, anyhow};
use ethereum_types::{H256, U256, Address};
use std::str::FromStr;

pub fn parse_hash(hex_str: &str) -> Result<H256> {
    let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    if cleaned.len() != 64 {
        return Err(anyhow!("Hash must be 64 hex characters (32 bytes)"));
    }

    let bytes = hex::decode(cleaned)?;
    Ok(H256::from_slice(&bytes))
}

pub fn parse_address(hex_str: &str) -> Result<Address> {
    let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    if cleaned.len() != 40 {
        return Err(anyhow!("Address must be 40 hex characters (20 bytes)"));
    }

    let bytes = hex::decode(cleaned)?;
    Ok(Address::from_slice(&bytes))
}

pub fn parse_u256(hex_str: &str) -> Result<U256> {
    let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    U256::from_str_radix(cleaned, 16).map_err(|e| anyhow!("Invalid hex number: {}", e))
}

pub fn parse_u64(hex_str: &str) -> Result<u64> {
    let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    u64::from_str_radix(cleaned, 16).map_err(|e| anyhow!("Invalid hex number: {}", e))
}

pub fn parse_hex_data(hex_str: &str) -> Result<Vec<u8>> {
    let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    hex::decode(cleaned).map_err(|e| anyhow!("Invalid hex data: {}", e))
}

pub fn format_hash(hash: &H256) -> String {
    format!("0x{}", hex::encode(hash.as_bytes()))
}

pub fn format_address(address: &Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

pub fn format_u256(value: &U256) -> String {
    format!("0x{:x}", value)
}

pub fn format_u64(value: u64) -> String {
    format!("0x{:x}", value)
}

pub fn format_hex_data(data: &[u8]) -> String {
    format!("0x{}", hex::encode(data))
}

pub fn is_valid_address(address_str: &str) -> bool {
    parse_address(address_str).is_ok()
}

pub fn is_valid_hash(hash_str: &str) -> bool {
    parse_hash(hash_str).is_ok()
}

pub fn generate_node_id() -> String {
    format!("rpc-node-{}", uuid::Uuid::new_v4())
}

pub fn setup_logging(level: &str) {
    let log_level = match level.to_lowercase().as_str() {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();
}

pub fn default_data_dir() -> std::path::PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push(".lightweight-rpc-node");
    path
}

pub fn ensure_directory(path: &std::path::Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
        log::info!("Created directory: {}", path.display());
    }
    Ok(())
}

pub fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn validate_config(config: &crate::config::Config) -> Result<()> {
    config.validate()
}

pub async fn is_address_reachable(addr: &str) -> bool {
    match tokio::net::TcpStream::connect(addr).await {
        Ok(_) => true,
        Err(_) => false,
    }
}
