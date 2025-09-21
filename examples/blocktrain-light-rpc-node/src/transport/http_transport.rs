use async_trait::async_trait;
use anyhow::{Result, anyhow, Context};
use reqwest::{Client, ClientBuilder};
use serde_json::{json, Value};
use ethereum_types::{H256, U256, Address};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use parking_lot::Mutex;
use crate::transport::Transport;
use crate::blockchain_state::{BlockInfo, TransactionInfo};

#[derive(Debug)]
pub struct HttpTransport {
    client: Client,
    rpc_url: String,
    chain_id: u64,
    timeout: Duration,
    max_retries: u32,

    // Health tracking
    request_count: AtomicU64,
    success_count: AtomicU64,
    error_count: AtomicU32,
    last_success: Mutex<Option<Instant>>,
    last_error: Mutex<Option<(Instant, String)>>,

    // Connection statistics
    avg_response_time: Mutex<Duration>,
    consecutive_failures: AtomicU32,
}

impl HttpTransport {
    pub fn new(rpc_url: String, chain_id: u64, timeout_ms: u64) -> Self {
        // Build client with connection pooling and better defaults
        let client = ClientBuilder::new()
            .timeout(Duration::from_millis(timeout_ms))
            .connect_timeout(Duration::from_millis(timeout_ms / 2))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .tcp_keepalive(Duration::from_secs(60))
            .http2_keep_alive_timeout(Duration::from_secs(30))
            .http2_keep_alive_interval(Duration::from_secs(10))
            .user_agent("lightweight-rpc-node/0.1.0")
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            rpc_url,
            chain_id,
            timeout: Duration::from_millis(timeout_ms),
            max_retries: 3,
            request_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            error_count: AtomicU32::new(0),
            last_success: Mutex::new(None),
            last_error: Mutex::new(None),
            avg_response_time: Mutex::new(Duration::from_millis(1000)),
            consecutive_failures: AtomicU32::new(0),
        }
    }

    async fn make_rpc_call(&self, method: &str, params: Value) -> Result<Value> {
        self.make_rpc_call_with_retries(method, params, self.max_retries).await
    }

    async fn make_rpc_call_with_retries(&self, method: &str, params: Value, retries_left: u32) -> Result<Value> {
        let start_time = Instant::now();
        self.request_count.fetch_add(1, Ordering::Relaxed);

        let request_id = self.request_count.load(Ordering::Relaxed);
        let request_body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": request_id
        });

        log::debug!("RPC call: {} to {} (attempt {})", method, self.rpc_url, self.max_retries - retries_left + 1);

        let result = async {
            let response = self
                .client
                .post(&self.rpc_url)
                .json(&request_body)
                .send()
                .await
                .with_context(|| format!("HTTP request failed for method {}", method))?;

            let status = response.status();
            if !status.is_success() {
                return Err(anyhow!("HTTP error {}: {}", status, status.canonical_reason().unwrap_or("Unknown")));
            }

            let response_body: Value = response.json().await
                .with_context(|| "Failed to parse JSON response")?;

            if let Some(error) = response_body.get("error") {
                let error_msg = error.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown RPC error");
                let error_code = error.get("code")
                    .and_then(|c| c.as_i64())
                    .unwrap_or(-1);

                return Err(anyhow!("RPC error {}: {}", error_code, error_msg));
            }

            response_body.get("result")
                .ok_or_else(|| anyhow!("No result field in response"))
                .map(|r| r.clone())
        }.await;

        let response_time = start_time.elapsed();

        match result {
            Ok(value) => {
                // Record success
                self.success_count.fetch_add(1, Ordering::Relaxed);
                self.consecutive_failures.store(0, Ordering::Relaxed);
                *self.last_success.lock() = Some(Instant::now());

                // Update average response time
                let mut avg_time = self.avg_response_time.lock();
                *avg_time = Duration::from_millis(
                    (avg_time.as_millis() as u64 + response_time.as_millis() as u64) / 2
                );

                log::debug!("RPC {} succeeded in {:?}", method, response_time);
                Ok(value)
            }
            Err(e) => {
                // Record failure
                self.error_count.fetch_add(1, Ordering::Relaxed);
                let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
                *self.last_error.lock() = Some((Instant::now(), e.to_string()));

                log::debug!("RPC {} failed: {} (failures: {})", method, e, failures);

                // Retry logic
                if retries_left > 0 && self.should_retry(&e, failures) {
                    let delay = self.calculate_retry_delay(self.max_retries - retries_left + 1);
                    log::debug!("Retrying {} in {:?} ({} retries left)", method, delay, retries_left - 1);

                    tokio::time::sleep(delay).await;
                    return self.make_rpc_call_with_retries(method, params, retries_left - 1).await;
                }

                Err(e)
            }
        }
    }

    fn should_retry(&self, error: &anyhow::Error, consecutive_failures: u32) -> bool {
        // Don't retry if too many consecutive failures
        if consecutive_failures > 10 {
            return false;
        }

        let error_str = error.to_string().to_lowercase();

        // Retry on network errors
        if error_str.contains("timeout") || 
           error_str.contains("connection") ||
           error_str.contains("network") ||
           error_str.contains("dns") {
            return true;
        }

        // Retry on certain HTTP status codes
        if error_str.contains("500") ||  // Internal server error
           error_str.contains("502") ||  // Bad gateway
           error_str.contains("503") ||  // Service unavailable
           error_str.contains("504") {   // Gateway timeout
            return true;
        }

        // Don't retry on RPC errors or client errors (4xx)
        false
    }

    fn calculate_retry_delay(&self, attempt: u32) -> Duration {
        // Exponential backoff with jitter
        let base_delay = Duration::from_millis(100 * (1 << attempt.min(6))); // Cap at ~6.4 seconds
        let jitter = Duration::from_millis(fastrand::u64(0..=base_delay.as_millis() as u64 / 4));
        base_delay + jitter
    }

    fn parse_hex_to_u64(hex_str: &str) -> Result<u64> {
        let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        if cleaned.is_empty() {
            return Ok(0);
        }
        u64::from_str_radix(cleaned, 16)
            .with_context(|| format!("Invalid hex number: {}", hex_str))
    }

    fn parse_hex_to_u256(hex_str: &str) -> Result<U256> {
        let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        if cleaned.is_empty() {
            return Ok(U256::zero());
        }
        U256::from_str_radix(cleaned, 16)
            .with_context(|| format!("Invalid hex U256: {}", hex_str))
    }

    fn parse_hex_to_h256(hex_str: &str) -> Result<H256> {
        let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        if cleaned.len() != 64 {
            return Err(anyhow!("Invalid H256 length: expected 64 chars, got {}", cleaned.len()));
        }
        let bytes = hex::decode(cleaned)
            .with_context(|| format!("Invalid hex data: {}", hex_str))?;
        Ok(H256::from_slice(&bytes))
    }

    fn parse_hex_to_address(hex_str: &str) -> Result<Address> {
        let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        if cleaned.len() != 40 {
            return Err(anyhow!("Invalid address length: expected 40 chars, got {}", cleaned.len()));
        }
        let bytes = hex::decode(cleaned)
            .with_context(|| format!("Invalid address hex: {}", hex_str))?;
        Ok(Address::from_slice(&bytes))
    }

    fn validate_block_response(block: &serde_json::Map<String, Value>) -> Result<()> {
        let required_fields = ["number", "hash", "parentHash", "timestamp", "gasLimit", "gasUsed"];
        for field in &required_fields {
            if !block.contains_key(*field) {
                return Err(anyhow!("Block missing required field: {}", field));
            }
        }
        Ok(())
    }

    fn validate_transaction_response(tx: &serde_json::Map<String, Value>) -> Result<()> {
        let required_fields = ["hash", "blockNumber", "transactionIndex", "from", "value", "gas", "gasPrice", "nonce"];
        for field in &required_fields {
            if !tx.contains_key(*field) {
                return Err(anyhow!("Transaction missing required field: {}", field));
            }
        }
        Ok(())
    }

    pub fn get_connection_stats(&self) -> ConnectionStats {
        let total_requests = self.request_count.load(Ordering::Relaxed);
        let success_count = self.success_count.load(Ordering::Relaxed);
        let error_count = self.error_count.load(Ordering::Relaxed) as u64;

        ConnectionStats {
            total_requests,
            success_count,
            error_count,
            success_rate: if total_requests > 0 {
                success_count as f32 / total_requests as f32
            } else {
                0.0
            },
            avg_response_time: *self.avg_response_time.lock(),
            consecutive_failures: self.consecutive_failures.load(Ordering::Relaxed),
            last_success: *self.last_success.lock(),
            last_error_time: self.last_error.lock().as_ref().map(|(time, _)| *time),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub total_requests: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub success_rate: f32,
    pub avg_response_time: Duration,
    pub consecutive_failures: u32,
    pub last_success: Option<Instant>,
    pub last_error_time: Option<Instant>,
}

#[async_trait]
impl Transport for HttpTransport {
    async fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing HTTP transport for RPC: {}", self.rpc_url);

        // Test connection and validate chain ID
        let result = self.make_rpc_call("eth_chainId", json!([])).await
            .with_context(|| "Failed to connect to RPC endpoint")?;

        let chain_id_hex = result.as_str()
            .ok_or_else(|| anyhow!("Invalid chain ID response format"))?;
        let reported_chain_id = Self::parse_hex_to_u64(chain_id_hex)
            .with_context(|| "Failed to parse chain ID")?;

        if reported_chain_id != self.chain_id {
            log::warn!(
                "Chain ID mismatch: expected {}, got {}. This may cause issues.",
                self.chain_id,
                reported_chain_id
            );
        }

        log::info!("HTTP transport initialized successfully. Chain ID: {}", reported_chain_id);
        Ok(())
    }

    async fn is_available(&self) -> bool {
        // Quick health check - don't retry on failure for availability check
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            self.make_rpc_call_with_retries("eth_chainId", json!([]), 0)
        ).await;

        match result {
            Ok(Ok(_)) => true,
            Ok(Err(e)) => {
                log::debug!("HTTP transport availability check failed: {}", e);
                false
            }
            Err(_) => {
                log::debug!("HTTP transport availability check timed out");
                false
            }
        }
    }

    async fn get_latest_block_number(&self) -> Result<u64> {
        let result = self.make_rpc_call("eth_blockNumber", json!([])).await
            .with_context(|| "Failed to get latest block number")?;
        let hex_str = result.as_str()
            .ok_or_else(|| anyhow!("Invalid block number response format"))?;
        Self::parse_hex_to_u64(hex_str)
    }

    async fn get_block_by_number(&self, number: u64) -> Result<Option<BlockInfo>> {
        let block_param = if number == 0 {
            "latest".to_string()
        } else {
            format!("0x{:x}", number)
        };

        let result = self.make_rpc_call("eth_getBlockByNumber", json!([block_param, true])).await
            .with_context(|| format!("Failed to get block {}", number))?;

        if result.is_null() {
            return Ok(None);
        }

        let block = result.as_object()
            .ok_or_else(|| anyhow!("Block response is not an object"))?;

        // Validate required fields
        Self::validate_block_response(block)
            .with_context(|| format!("Invalid block {} response", number))?;

        let block_info = BlockInfo {
            number: Self::parse_hex_to_u64(
                block.get("number")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing or invalid block number"))?
            ).with_context(|| "Failed to parse block number")?,

            hash: Self::parse_hex_to_h256(
                block.get("hash")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing or invalid block hash"))?
            ).with_context(|| "Failed to parse block hash")?,

            parent_hash: Self::parse_hex_to_h256(
                block.get("parentHash")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing or invalid parent hash"))?
            ).with_context(|| "Failed to parse parent hash")?,

            timestamp: Self::parse_hex_to_u64(
                block.get("timestamp")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing or invalid timestamp"))?
            ).with_context(|| "Failed to parse timestamp")?,

            transactions: block.get("transactions")
                .and_then(|v| v.as_array())
                .ok_or_else(|| anyhow!("Missing or invalid transactions array"))?
                .iter()
                .filter_map(|tx| {
                    tx.as_object()
                        .and_then(|obj| obj.get("hash"))
                        .and_then(|h| h.as_str())
                        .and_then(|s| Self::parse_hex_to_h256(s).ok())
                })
                .collect(),

            state_root: Self::parse_hex_to_h256(
                block.get("stateRoot")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing or invalid state root"))?
            ).with_context(|| "Failed to parse state root")?,

            receipts_root: Self::parse_hex_to_h256(
                block.get("receiptsRoot")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing or invalid receipts root"))?
            ).with_context(|| "Failed to parse receipts root")?,

            gas_limit: Self::parse_hex_to_u256(
                block.get("gasLimit")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing or invalid gas limit"))?
            ).with_context(|| "Failed to parse gas limit")?,

            gas_used: Self::parse_hex_to_u256(
                block.get("gasUsed")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing or invalid gas used"))?
            ).with_context(|| "Failed to parse gas used")?,

            size: block.get("size")
                .and_then(|v| v.as_str())
                .and_then(|s| Self::parse_hex_to_u64(s).ok()),
        };

        Ok(Some(block_info))
    }

    async fn get_block_by_hash(&self, hash: &H256) -> Result<Option<BlockInfo>> {
        let hash_str = format!("0x{}", hex::encode(hash.as_bytes()));
        let result = self.make_rpc_call("eth_getBlockByHash", json!([hash_str, true])).await
            .with_context(|| format!("Failed to get block by hash {}", hex::encode(hash)))?;

        if result.is_null() {
            return Ok(None);
        }

        // Use the same logic as get_block_by_number
        let block = result.as_object()
            .ok_or_else(|| anyhow!("Block response is not an object"))?;
        Self::validate_block_response(block)?;

        // Parse block (similar to get_block_by_number, but we know the hash)
        let block_info = BlockInfo {
            hash: *hash, // We already know the hash
            number: Self::parse_hex_to_u64(
                block.get("number").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing block number"))?
            )?,
            parent_hash: Self::parse_hex_to_h256(
                block.get("parentHash").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing parent hash"))?
            )?,
            timestamp: Self::parse_hex_to_u64(
                block.get("timestamp").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing timestamp"))?
            )?,
            transactions: block.get("transactions")
                .and_then(|v| v.as_array())
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|tx| {
                    tx.as_object()
                        .and_then(|obj| obj.get("hash"))
                        .and_then(|h| h.as_str())
                        .and_then(|s| Self::parse_hex_to_h256(s).ok())
                })
                .collect(),
            state_root: Self::parse_hex_to_h256(
                block.get("stateRoot").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing state root"))?
            )?,
            receipts_root: Self::parse_hex_to_h256(
                block.get("receiptsRoot").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing receipts root"))?
            )?,
            gas_limit: Self::parse_hex_to_u256(
                block.get("gasLimit").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing gas limit"))?
            )?,
            gas_used: Self::parse_hex_to_u256(
                block.get("gasUsed").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing gas used"))?
            )?,
            size: block.get("size")
                .and_then(|v| v.as_str())
                .and_then(|s| Self::parse_hex_to_u64(s).ok()),
        };

        Ok(Some(block_info))
    }

    async fn get_transaction(&self, hash: &H256) -> Result<Option<TransactionInfo>> {
        let hash_str = format!("0x{}", hex::encode(hash.as_bytes()));
        let result = self.make_rpc_call("eth_getTransactionByHash", json!([hash_str])).await
            .with_context(|| format!("Failed to get transaction {}", hex::encode(hash)))?;

        if result.is_null() {
            return Ok(None);
        }

        let tx = result.as_object()
            .ok_or_else(|| anyhow!("Transaction response is not an object"))?;

        Self::validate_transaction_response(tx)
            .with_context(|| format!("Invalid transaction {} response", hex::encode(hash)))?;

        let tx_info = TransactionInfo {
            hash: *hash,
            block_number: Self::parse_hex_to_u64(
                tx.get("blockNumber").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing block number"))?
            )?,
            transaction_index: Self::parse_hex_to_u64(
                tx.get("transactionIndex").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing transaction index"))?
            )?,
            from: Self::parse_hex_to_address(
                tx.get("from").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing from address"))?
            )?,
            to: tx.get("to")
                .and_then(|v| v.as_str())
                .map(|s| Self::parse_hex_to_address(s))
                .transpose()?,
            value: Self::parse_hex_to_u256(
                tx.get("value").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing value"))?
            )?,
            gas: Self::parse_hex_to_u256(
                tx.get("gas").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing gas"))?
            )?,
            gas_price: Self::parse_hex_to_u256(
                tx.get("gasPrice").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing gas price"))?
            )?,
            input: {
                let input_str = tx.get("input").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing input"))?;
                let cleaned = input_str.strip_prefix("0x").unwrap_or(input_str);
                hex::decode(cleaned).with_context(|| "Invalid input hex data")?
            },
            nonce: Self::parse_hex_to_u64(
                tx.get("nonce").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("Missing nonce"))?
            )?,
            receipt: None, // Will be fetched separately if needed
            status: None,
        };

        Ok(Some(tx_info))
    }

    async fn get_balance(&self, address: &Address) -> Result<U256> {
        let address_str = format!("0x{}", hex::encode(address.as_bytes()));
        let result = self.make_rpc_call("eth_getBalance", json!([address_str, "latest"])).await
            .with_context(|| format!("Failed to get balance for {}", hex::encode(address)))?;
        let hex_str = result.as_str()
            .ok_or_else(|| anyhow!("Invalid balance response format"))?;
        Self::parse_hex_to_u256(hex_str)
    }

    async fn get_transaction_count(&self, address: &Address) -> Result<U256> {
        let address_str = format!("0x{}", hex::encode(address.as_bytes()));
        let result = self.make_rpc_call("eth_getTransactionCount", json!([address_str, "latest"])).await
            .with_context(|| format!("Failed to get transaction count for {}", hex::encode(address)))?;
        let hex_str = result.as_str()
            .ok_or_else(|| anyhow!("Invalid transaction count response format"))?;
        let count = Self::parse_hex_to_u64(hex_str)?;
        Ok(U256::from(count))
    }

    async fn send_raw_transaction(&self, tx_data: &[u8]) -> Result<H256> {
        let tx_hex = format!("0x{}", hex::encode(tx_data));
        let result = self.make_rpc_call("eth_sendRawTransaction", json!([tx_hex])).await
            .with_context(|| "Failed to send raw transaction")?;
        let hash_str = result.as_str()
            .ok_or_else(|| anyhow!("Invalid transaction hash response format"))?;
        Self::parse_hex_to_h256(hash_str)
    }

    async fn call_contract(&self, to: &Address, data: &[u8]) -> Result<Vec<u8>> {
        let to_str = format!("0x{}", hex::encode(to.as_bytes()));
        let data_str = format!("0x{}", hex::encode(data));

        let call_object = json!({
            "to": to_str,
            "data": data_str
        });

        let result = self.make_rpc_call("eth_call", json!([call_object, "latest"])).await
            .with_context(|| format!("Failed to call contract at {}", hex::encode(to)))?;
        let hex_str = result.as_str()
            .ok_or_else(|| anyhow!("Invalid call response format"))?;
        let cleaned = hex_str.strip_prefix("0x").unwrap_or(hex_str);
        hex::decode(cleaned).with_context(|| "Invalid hex in call response")
    }

    async fn estimate_gas(&self, to: Option<&Address>, data: &[u8]) -> Result<U256> {
        let mut call_object = json!({
            "data": format!("0x{}", hex::encode(data))
        });

        if let Some(to_addr) = to {
            call_object["to"] = json!(format!("0x{}", hex::encode(to_addr.as_bytes())));
        }

        let result = self.make_rpc_call("eth_estimateGas", json!([call_object])).await
            .with_context(|| "Failed to estimate gas")?;
        let hex_str = result.as_str()
            .ok_or_else(|| anyhow!("Invalid gas estimate response format"))?;
        Self::parse_hex_to_u256(hex_str)
    }

    async fn get_gas_price(&self) -> Result<U256> {
        let result = self.make_rpc_call("eth_gasPrice", json!([])).await
            .with_context(|| "Failed to get gas price")?;
        let hex_str = result.as_str()
            .ok_or_else(|| anyhow!("Invalid gas price response format"))?;
        Self::parse_hex_to_u256(hex_str)
    }

    async fn get_health_score(&self) -> f32 {
        let stats = self.get_connection_stats();

        // Base score from success rate
        let mut score = stats.success_rate;

        // Penalty for consecutive failures
        if stats.consecutive_failures > 5 {
            score -= 0.3;
        } else if stats.consecutive_failures > 2 {
            score -= 0.1;
        }

        // Penalty for slow responses
        if stats.avg_response_time > Duration::from_secs(10) {
            score -= 0.3;
        } else if stats.avg_response_time > Duration::from_secs(5) {
            score -= 0.1;
        }

        // Penalty for old last success
        if let Some(last_success) = stats.last_success {
            if last_success.elapsed() > Duration::from_secs(300) { // 5 minutes
                score -= 0.4;
            } else if last_success.elapsed() > Duration::from_secs(60) { // 1 minute
                score -= 0.2;
            }
        } else {
            score -= 0.5; // Never succeeded
        }

        score.max(0.0).min(1.0)
    }

    async fn shutdown(&mut self) -> Result<()> {
        log::info!("Shutting down HTTP transport");
        // reqwest client doesn't need explicit shutdown, but we can log stats
        let stats = self.get_connection_stats();
        log::info!("HTTP transport stats - Requests: {}, Success rate: {:.1}%, Avg response: {:?}",
                  stats.total_requests, stats.success_rate * 100.0, stats.avg_response_time);
        Ok(())
    }
}

// Add fastrand for jitter calculation
mod fastrand {
    use std::cell::Cell;
    use std::num::Wrapping;

    thread_local! {
        static RNG: Cell<Wrapping<u64>> = Cell::new(Wrapping(1));
    }

    pub fn u64(range: std::ops::RangeInclusive<u64>) -> u64 {
        RNG.with(|rng| {
            let mut x = rng.get();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            rng.set(x);
            range.start() + (x.0 % (range.end() - range.start() + 1))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_parsing() {
        assert_eq!(HttpTransport::parse_hex_to_u64("0x1234").unwrap(), 0x1234);
        assert_eq!(HttpTransport::parse_hex_to_u64("1234").unwrap(), 0x1234);
        assert_eq!(HttpTransport::parse_hex_to_u64("0x").unwrap(), 0);
        assert_eq!(HttpTransport::parse_hex_to_u64("").unwrap(), 0);
    }

    #[test]
    fn test_transport_creation() {
        let transport = HttpTransport::new(
            "http://blocktrain.local:8545".to_string(),
            31337,
            30000,
        );
        assert_eq!(transport.rpc_url, "http://blocktrain.local:8545");
        assert_eq!(transport.chain_id, 31337);
        assert_eq!(transport.request_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_retry_decision() {
        let transport = HttpTransport::new("http://test".to_string(), 1, 1000);

        let timeout_error = anyhow!("timeout");
        assert!(transport.should_retry(&timeout_error, 1));

        let rpc_error = anyhow!("RPC error -32000: insufficient funds");
        assert!(!transport.should_retry(&rpc_error, 1));

        // Too many failures
        let network_error = anyhow!("connection failed");
        assert!(!transport.should_retry(&network_error, 15));
    }
}
