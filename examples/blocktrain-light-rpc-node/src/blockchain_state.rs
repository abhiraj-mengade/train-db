use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock; // Better than tokio::sync::RwLock for this use case
use serde::{Deserialize, Serialize};
use ethereum_types::{H256, U256, Address};
use ethers::types::{Block, Transaction, TransactionReceipt};
use anyhow::{Result, anyhow, Context};
use rocksdb::{DB, Options, WriteBatch, IteratorMode};
use std::path::Path;
use dashmap::DashMap; // Thread-safe HashMap for better concurrency

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainState {
    pub latest_block_number: u64,
    pub latest_block_hash: H256,
    pub chain_id: u64,
    pub gas_price: U256,
    pub sync_status: SyncStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockInfo {
    pub number: u64,
    pub hash: H256,
    pub parent_hash: H256,
    pub timestamp: u64,
    pub transactions: Vec<H256>,
    pub state_root: H256,
    pub receipts_root: H256,
    pub gas_limit: U256,
    pub gas_used: U256,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub hash: H256,
    pub block_number: u64,
    pub transaction_index: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas: U256,
    pub gas_price: U256,
    pub input: Vec<u8>,
    pub nonce: u64,
    pub receipt: Option<TransactionReceiptInfo>,
    pub status: Option<u8>, // 0 = failed, 1 = success
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionReceiptInfo {
    pub transaction_hash: H256,
    pub block_number: u64,
    pub gas_used: U256,
    pub status: Option<u64>,
    pub logs: Vec<LogInfo>,
    pub contract_address: Option<Address>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogInfo {
    pub address: Address,
    pub topics: Vec<H256>,
    pub data: Vec<u8>,
    pub log_index: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub nonce: U256,
    pub balance: U256,
    pub storage_root: H256,
    pub code_hash: H256,
    pub code: Option<Vec<u8>>, // Store contract code if available
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SyncStatus {
    NotSynced,
    Syncing { current: u64, highest: u64, peers: u32 },
    Synced,
    Failed { error: String, retry_count: u32 },
}

// Thread-safe state manager with better concurrency
#[derive(Debug)]
pub struct StateManager {
    // Use Arc<RwLock<>> for the core state
    state: Arc<RwLock<BlockchainState>>,
    db: Arc<DB>,

    // Use DashMap for better concurrent access to cached data
    block_cache: DashMap<u64, BlockInfo>,
    transaction_cache: DashMap<H256, TransactionInfo>, 
    account_cache: DashMap<Address, AccountState>,

    // Configuration
    max_cache_blocks: usize,
    max_cache_transactions: usize,
    max_cache_accounts: usize,
}

impl StateManager {
    pub fn new(db_path: &Path, chain_id: u64) -> Result<Self> {
        // Better RocksDB configuration
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_max_open_files(1000); // Reduced from 10000
        opts.set_use_fsync(false);
        opts.set_bytes_per_sync(1048576); // 1MB
        opts.optimize_for_point_lookup(256); // 256MB block cache
        opts.set_target_file_size_base(67108864); // 64MB
        opts.set_write_buffer_size(134217728); // 128MB
        opts.set_max_write_buffer_number(3);
        opts.set_level_zero_file_num_compaction_trigger(8);
        opts.set_level_zero_slowdown_writes_trigger(17);
        opts.set_level_zero_stop_writes_trigger(24);
        opts.set_num_levels(4);
        opts.set_max_bytes_for_level_base(536870912); // 512MB
        opts.set_max_bytes_for_level_multiplier(8.0);

        let db = Arc::new(DB::open(&opts, db_path)
            .with_context(|| format!("Failed to open database at {}", db_path.display()))?);

        // Load existing state from database
        let initial_state = Self::load_state_from_db(&db, chain_id)?;

        let state_manager = Self {
            state: Arc::new(RwLock::new(initial_state)),
            db,
            block_cache: DashMap::new(),
            transaction_cache: DashMap::new(), 
            account_cache: DashMap::new(),
            max_cache_blocks: 1000,
            max_cache_transactions: 10000,
            max_cache_accounts: 5000,
        };

        // Load recent data into cache
        state_manager.warm_cache()?;

        Ok(state_manager)
    }

    fn load_state_from_db(db: &DB, chain_id: u64) -> Result<BlockchainState> {
        let state_key = b"blockchain_state";

        match db.get(state_key)? {
            Some(data) => {
                bincode::deserialize(&data)
                    .with_context(|| "Failed to deserialize blockchain state")
            }
            None => {
                // Create new state
                Ok(BlockchainState {
                    latest_block_number: 0,
                    latest_block_hash: H256::zero(),
                    chain_id,
                    gas_price: U256::from(20_000_000_000u64), // 20 gwei default
                    sync_status: SyncStatus::NotSynced,
                })
            }
        }
    }

    fn warm_cache(&self) -> Result<()> {
        let state = self.state.read();
        let latest_block = state.latest_block_number;
        drop(state);

        if latest_block == 0 {
            return Ok(());
        }

        // Load recent blocks into cache
        let start_block = latest_block.saturating_sub(100);
        for block_num in start_block..=latest_block {
            if let Ok(Some(block)) = self.load_block_from_db(block_num) {
                self.block_cache.insert(block_num, block);
            }
        }

        log::info!("Warmed cache with {} blocks", self.block_cache.len());
        Ok(())
    }

    pub fn get_latest_block_number(&self) -> u64 {
        self.state.read().latest_block_number
    }

    pub fn get_block_by_number(&self, number: u64) -> Option<BlockInfo> {
        // Check cache first
        if let Some(block) = self.block_cache.get(&number) {
            return Some(block.clone());
        }

        // Load from database
        match self.load_block_from_db(number) {
            Ok(Some(block)) => {
                // Add to cache if there's room
                self.maybe_cache_block(number, block.clone());
                Some(block)
            }
            _ => None,
        }
    }

    pub fn get_block_by_hash(&self, hash: &H256) -> Option<BlockInfo> {
        // Search cache first
        for entry in self.block_cache.iter() {
            if entry.hash == *hash {
                return Some(entry.clone());
            }
        }

        // Search database - this is expensive, but we need it
        self.find_block_by_hash_in_db(hash)
    }

    fn find_block_by_hash_in_db(&self, hash: &H256) -> Option<BlockInfo> {
        // Iterate through block range to find the hash
        let state = self.state.read();
        let latest = state.latest_block_number;
        drop(state);

        // Search recent blocks first (more likely to be found)
        for block_num in (latest.saturating_sub(1000)..=latest).rev() {
            if let Ok(Some(block)) = self.load_block_from_db(block_num) {
                if block.hash == *hash {
                    self.maybe_cache_block(block_num, block.clone());
                    return Some(block);
                }
            }
        }
        None
    }

    fn load_block_from_db(&self, number: u64) -> Result<Option<BlockInfo>> {
        let key = format!("block:{}", number);
        match self.db.get(key.as_bytes())? {
            Some(data) => {
                let block: BlockInfo = bincode::deserialize(&data)
                    .with_context(|| format!("Failed to deserialize block {}", number))?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    fn maybe_cache_block(&self, number: u64, block: BlockInfo) {
        if self.block_cache.len() < self.max_cache_blocks {
            self.block_cache.insert(number, block);
        }
    }

    pub fn get_transaction(&self, hash: &H256) -> Option<TransactionInfo> {
        // Check cache first
        if let Some(tx) = self.transaction_cache.get(hash) {
            return Some(tx.clone());
        }

        // Load from database
        match self.load_transaction_from_db(hash) {
            Ok(Some(tx)) => {
                self.maybe_cache_transaction(hash, tx.clone());
                Some(tx)
            }
            _ => None,
        }
    }

    fn load_transaction_from_db(&self, hash: &H256) -> Result<Option<TransactionInfo>> {
        let key = format!("tx:{}", hex::encode(hash.as_bytes()));
        match self.db.get(key.as_bytes())? {
            Some(data) => {
                let tx: TransactionInfo = bincode::deserialize(&data)
                    .with_context(|| format!("Failed to deserialize transaction {}", hex::encode(hash)))?;
                Ok(Some(tx))
            }
            None => Ok(None),
        }
    }

    fn maybe_cache_transaction(&self, hash: &H256, tx: TransactionInfo) {
        if self.transaction_cache.len() < self.max_cache_transactions {
            self.transaction_cache.insert(*hash, tx);
        }
    }

    pub fn get_account_balance(&self, address: &Address) -> U256 {
        self.account_cache.get(address)
            .map(|acc| acc.balance)
            .unwrap_or_else(|| {
                // Try to load from database
                self.load_account_from_db(address)
                    .unwrap_or_default()
                    .balance
            })
    }

    pub fn get_account_nonce(&self, address: &Address) -> U256 {
        self.account_cache.get(address)
            .map(|acc| acc.nonce)
            .unwrap_or_else(|| {
                self.load_account_from_db(address)
                    .unwrap_or_default()
                    .nonce
            })
    }

    fn load_account_from_db(&self, address: &Address) -> Result<AccountState> {
        let key = format!("account:{}", hex::encode(address.as_bytes()));
        match self.db.get(key.as_bytes())? {
            Some(data) => {
                bincode::deserialize(&data)
                    .with_context(|| format!("Failed to deserialize account {}", hex::encode(address)))
            }
            None => Ok(AccountState::default()),
        }
    }

    pub fn get_gas_price(&self) -> U256 {
        self.state.read().gas_price
    }

    pub fn get_sync_status(&self) -> SyncStatus {
        self.state.read().sync_status.clone()
    }

    // Batch update method for better performance
    pub fn batch_update_blocks(&self, blocks: Vec<BlockInfo>) -> Result<()> {
        if blocks.is_empty() {
            return Ok(());
        }

        let mut batch = WriteBatch::default();
        let mut latest_block_number = 0;
        let mut latest_block_hash = H256::zero();

        for block in blocks {
            // Update latest if this block is newer
            if block.number > latest_block_number {
                latest_block_number = block.number;
                latest_block_hash = block.hash;
            }

            // Serialize and add to batch
            let key = format!("block:{}", block.number);
            let value = bincode::serialize(&block)
                .with_context(|| format!("Failed to serialize block {}", block.number))?;
            batch.put(key.as_bytes(), &value);

            // Add to cache
            self.maybe_cache_block(block.number, block);
        }

        // Update state if we have a newer block
        if latest_block_number > 0 {
            let mut state = self.state.write();
            if latest_block_number > state.latest_block_number {
                state.latest_block_number = latest_block_number;
                state.latest_block_hash = latest_block_hash;

                // Serialize and save state
                let state_data = bincode::serialize(&*state)
                    .context("Failed to serialize blockchain state")?;
                batch.put(b"blockchain_state", &state_data);
            }
        }

        // Write batch atomically
        self.db.write(batch)
            .context("Failed to write block batch to database")?;

        Ok(())
    }

    pub fn update_block(&self, block_info: BlockInfo) -> Result<()> {
        self.batch_update_blocks(vec![block_info])
    }

    pub fn update_transaction(&self, tx_info: TransactionInfo) -> Result<()> {
        // Serialize and store in database
        let key = format!("tx:{}", hex::encode(tx_info.hash.as_bytes()));
        let value = bincode::serialize(&tx_info)
            .with_context(|| format!("Failed to serialize transaction {}", hex::encode(tx_info.hash)))?;

        self.db.put(key.as_bytes(), &value)
            .with_context(|| format!("Failed to store transaction {}", hex::encode(tx_info.hash)))?;

        // Add to cache
        self.maybe_cache_transaction(&tx_info.hash, tx_info);

        Ok(())
    }

    pub fn update_account(&self, address: Address, account: AccountState) -> Result<()> {
        // Serialize and store in database
        let key = format!("account:{}", hex::encode(address.as_bytes()));
        let value = bincode::serialize(&account)
            .with_context(|| format!("Failed to serialize account {}", hex::encode(address)))?;

        self.db.put(key.as_bytes(), &value)
            .with_context(|| format!("Failed to store account {}", hex::encode(address)))?;

        // Update cache
        if self.account_cache.len() < self.max_cache_accounts {
            self.account_cache.insert(address, account);
        }

        Ok(())
    }

    pub fn update_sync_status(&self, status: SyncStatus) {
        let mut state = self.state.write();
        state.sync_status = status;

        // Persist state change
        if let Ok(state_data) = bincode::serialize(&*state) {
            let _ = self.db.put(b"blockchain_state", &state_data);
        }
    }

    pub fn update_gas_price(&self, gas_price: U256) {
        let mut state = self.state.write();
        state.gas_price = gas_price;

        // Persist state change
        if let Ok(state_data) = bincode::serialize(&*state) {
            let _ = self.db.put(b"blockchain_state", &state_data);
        }
    }

    // Enhanced serialization for P2P sync with data validation
    pub fn serialize_state_for_sync(&self) -> Result<Vec<u8>> {
        let state = self.state.read();

        // Only sync recent blocks to avoid huge payloads
        let recent_blocks: Vec<BlockInfo> = self.block_cache.iter()
            .filter(|entry| entry.number > state.latest_block_number.saturating_sub(50))
            .map(|entry| entry.clone())
            .collect();

        let sync_data = SyncData {
            latest_block_number: state.latest_block_number,
            latest_block_hash: state.latest_block_hash,
            chain_id: state.chain_id,
            gas_price: state.gas_price,
            recent_blocks,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        bincode::serialize(&sync_data)
            .context("Failed to serialize state for P2P sync")
    }

    // Enhanced deserialization with validation
    pub fn apply_synced_state(&self, data: &[u8]) -> Result<bool> {
        let sync_data: SyncData = bincode::deserialize(data)
            .context("Failed to deserialize synced state data")?;

        // Validate data age (reject data older than 10 minutes)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if current_time.saturating_sub(sync_data.timestamp) > 600 {
            return Ok(false); // Data too old
        }

        // Validate chain ID
        let current_state = self.state.read();
        if sync_data.chain_id != current_state.chain_id {
            return Err(anyhow!("Chain ID mismatch: got {}, expected {}", 
                              sync_data.chain_id, current_state.chain_id));
        }

        // Only apply if the synced data is newer
        if sync_data.latest_block_number <= current_state.latest_block_number {
            return Ok(false); // No new data
        }

        drop(current_state);

        // Apply the blocks
        self.batch_update_blocks(sync_data.recent_blocks)?;

        // Update gas price if it's reasonable (within 10x of current)
        let current_gas = self.get_gas_price();
        if sync_data.gas_price > U256::zero() && 
           sync_data.gas_price < current_gas * U256::from(10) &&
           sync_data.gas_price > current_gas / U256::from(10) {
            self.update_gas_price(sync_data.gas_price);
        }

        log::info!("Applied synced state: {} new blocks", 
                   sync_data.recent_blocks.len());

        Ok(true)
    }

    // Cleanup method to prevent memory leaks
    pub fn cleanup_cache(&self) {
        let current_block = self.get_latest_block_number();

        // Remove old blocks from cache
        let cutoff_block = current_block.saturating_sub(self.max_cache_blocks as u64);
        self.block_cache.retain(|&block_num, _| block_num > cutoff_block);

        // Limit transaction cache size
        if self.transaction_cache.len() > self.max_cache_transactions {
            // Remove oldest transactions (simple approach - could be improved)
            let to_remove: Vec<H256> = self.transaction_cache.iter()
                .take(self.max_cache_transactions / 4)
                .map(|entry| *entry.key())
                .collect();

            for tx_hash in to_remove {
                self.transaction_cache.remove(&tx_hash);
            }
        }

        // Limit account cache size  
        if self.account_cache.len() > self.max_cache_accounts {
            let to_remove: Vec<Address> = self.account_cache.iter()
                .take(self.max_cache_accounts / 4)
                .map(|entry| *entry.key())
                .collect();

            for address in to_remove {
                self.account_cache.remove(&address);
            }
        }
    }

    // Database statistics
    pub fn get_db_stats(&self) -> Result<DatabaseStats> {
        let mut stats = DatabaseStats::default();

        // Count blocks
        for item in self.db.iterator(IteratorMode::From(b"block:", rocksdb::Direction::Forward)) {
            match item {
                Ok((key, _)) => {
                    if key.starts_with(b"block:") {
                        stats.block_count += 1;
                    } else {
                        break; // No more block keys
                    }
                }
                Err(_) => break,
            }
        }

        // Count transactions
        for item in self.db.iterator(IteratorMode::From(b"tx:", rocksdb::Direction::Forward)) {
            match item {
                Ok((key, _)) => {
                    if key.starts_with(b"tx:") {
                        stats.transaction_count += 1;
                    } else {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        stats.cache_block_count = self.block_cache.len();
        stats.cache_transaction_count = self.transaction_cache.len();
        stats.cache_account_count = self.account_cache.len();

        Ok(stats)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncData {
    latest_block_number: u64,
    latest_block_hash: H256,
    chain_id: u64,
    gas_price: U256,
    recent_blocks: Vec<BlockInfo>,
    timestamp: u64,
}

#[derive(Debug, Default)]
pub struct DatabaseStats {
    pub block_count: usize,
    pub transaction_count: usize,
    pub account_count: usize,
    pub cache_block_count: usize,
    pub cache_transaction_count: usize,
    pub cache_account_count: usize,
}

impl Default for AccountState {
    fn default() -> Self {
        Self {
            nonce: U256::zero(),
            balance: U256::zero(),
            storage_root: H256::zero(),
            code_hash: H256::zero(),
            code: None,
        }
    }
}

// Conversion utilities remain the same but with better error handling
impl TryFrom<ethers::types::Block<ethers::types::Transaction>> for BlockInfo {
    type Error = anyhow::Error;

    fn try_from(block: ethers::types::Block<ethers::types::Transaction>) -> Result<Self> {
        Ok(Self {
            number: block.number
                .ok_or_else(|| anyhow!("Block missing number"))?
                .as_u64(),
            hash: block.hash
                .ok_or_else(|| anyhow!("Block missing hash"))?,
            parent_hash: block.parent_hash,
            timestamp: block.timestamp.as_u64(),
            transactions: block.transactions.iter().map(|tx| tx.hash).collect(),
            state_root: block.state_root,
            receipts_root: block.receipts_root,
            gas_limit: block.gas_limit,
            gas_used: block.gas_used,
            size: block.size.map(|s| s.as_u64()),
        })
    }
}

impl TryFrom<ethers::types::Transaction> for TransactionInfo {
    type Error = anyhow::Error;

    fn try_from(tx: ethers::types::Transaction) -> Result<Self> {
        Ok(Self {
            hash: tx.hash,
            block_number: tx.block_number
                .ok_or_else(|| anyhow!("Transaction missing block number"))?
                .as_u64(),
            transaction_index: tx.transaction_index
                .ok_or_else(|| anyhow!("Transaction missing index"))?
                .as_u64(),
            from: tx.from,
            to: tx.to,
            value: tx.value,
            gas: tx.gas,
            gas_price: tx.gas_price.unwrap_or_default(),
            input: tx.input.to_vec(),
            nonce: tx.nonce,
            receipt: None,
            status: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_state_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let state_manager = StateManager::new(&temp_dir.path().join("test.db"), 31337);
        assert!(state_manager.is_ok());
    }

    #[test]
    fn test_sync_data_validation() {
        let temp_dir = TempDir::new().unwrap();
        let state_manager = StateManager::new(&temp_dir.path().join("test.db"), 31337).unwrap();

        // Test with invalid chain ID
        let invalid_sync_data = SyncData {
            chain_id: 999,
            latest_block_number: 100,
            latest_block_hash: H256::zero(),
            gas_price: U256::from(1000000000u64),
            recent_blocks: Vec::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let serialized = bincode::serialize(&invalid_sync_data).unwrap();
        let result = state_manager.apply_synced_state(&serialized);
        assert!(result.is_err());
    }
}
