use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;
use anyhow::{Result, anyhow, Context};
use parking_lot::RwLock;
use crate::config::Config;
use crate::blockchain_state::{StateManager, SyncStatus};
use crate::transport::{Transport, TransportManager};

pub struct SyncEngine {
    config: Config,
    state_manager: Arc<StateManager>,
    transport_manager: Arc<RwLock<TransportManager>>,
    is_running: Arc<RwLock<bool>>,
    last_sync: Arc<RwLock<Instant>>,
    sync_stats: Arc<RwLock<SyncStats>>,
}

#[derive(Debug, Default, Clone)]
pub struct SyncStats {
    pub total_blocks_synced: u64,
    pub total_sync_time: Duration,
    pub last_sync_duration: Duration,
    pub sync_errors: u32,
    pub successful_syncs: u32,
}

impl SyncEngine {
    pub fn new(
        config: Config,
        state_manager: Arc<StateManager>,
        transport_manager: Arc<RwLock<TransportManager>>,
    ) -> Self {
        Self {
            config,
            state_manager,
            transport_manager,
            is_running: Arc::new(RwLock::new(false)),
            last_sync: Arc::new(RwLock::new(Instant::now())),
            sync_stats: Arc::new(RwLock::new(SyncStats::default())),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write();
        if *is_running {
            return Err(anyhow!("Sync engine already running"));
        }
        *is_running = true;
        drop(is_running);

        log::info!("Starting sync engine");

        // Initialize transport manager
        {
            let mut transport = self.transport_manager.write();
            transport.initialize().await
                .context("Failed to initialize transport manager")?;
        }

        // Start sync loops
        self.start_sync_loop().await;
        self.start_health_check_loop().await;

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        log::info!("Stopping sync engine");
        *self.is_running.write() = false;

        // Shutdown transport
        {
            let mut transport = self.transport_manager.write();
            transport.shutdown().await
                .context("Failed to shutdown transport")?;
        }

        Ok(())
    }

    pub fn is_running(&self) -> bool {
        *self.is_running.read()
    }

    async fn start_sync_loop(&self) {
        let is_running = self.is_running.clone();
        let state_manager = self.state_manager.clone();
        let transport_manager = self.transport_manager.clone();
        let last_sync = self.last_sync.clone();
        let sync_stats = self.sync_stats.clone();
        let sync_interval = Duration::from_millis(self.config.transport.sync_interval_ms);

        tokio::spawn(async move {
            let mut interval = interval(sync_interval);

            while *is_running.read() {
                interval.tick().await;

                let sync_start = Instant::now();
                match Self::perform_sync_cycle(&state_manager, &transport_manager).await {
                    Ok(blocks_synced) => {
                        let sync_duration = sync_start.elapsed();
                        *last_sync.write() = Instant::now();

                        let mut stats = sync_stats.write();
                        stats.total_blocks_synced += blocks_synced;
                        stats.total_sync_time += sync_duration;
                        stats.last_sync_duration = sync_duration;
                        stats.successful_syncs += 1;
                        drop(stats);

                        if blocks_synced > 0 {
                            log::info!("Sync cycle completed: {} blocks in {:?}", blocks_synced, sync_duration);
                        } else {
                            log::debug!("Sync cycle completed: no new blocks");
                        }
                    }
                    Err(e) => {
                        log::error!("Sync cycle failed: {}", e);
                        sync_stats.write().sync_errors += 1;
                    }
                }
            }
        });
    }

    async fn start_health_check_loop(&self) {
        let is_running = self.is_running.clone();
        let transport_manager = self.transport_manager.clone();
        let last_sync = self.last_sync.clone();
        let state_manager = self.state_manager.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            while *is_running.read() {
                interval.tick().await;

                // Check transport health
                let is_available = {
                    let transport = transport_manager.read();
                    transport.is_available().await
                };

                if !is_available {
                    log::warn!("Transport not available - attempting recovery");
                    state_manager.update_sync_status(SyncStatus::Failed {
                        error: "Transport unavailable".to_string(),
                        retry_count: 0,
                    }).await;
                }

                // Check sync health
                let last_sync_time = *last_sync.read();
                let time_since_sync = last_sync_time.elapsed();

                if time_since_sync > Duration::from_secs(300) { // 5 minutes
                    log::warn!("No successful sync for {} seconds", time_since_sync.as_secs());
                    state_manager.update_sync_status(SyncStatus::Failed {
                        error: format!("No sync for {} seconds", time_since_sync.as_secs()),
                        retry_count: 0,
                    }).await;
                }
            }
        });
    }

    async fn perform_sync_cycle(
        state_manager: &Arc<StateManager>,
        transport_manager: &Arc<RwLock<TransportManager>>,
    ) -> Result<u64> {
        // Get current local state
        let local_block_number = state_manager.get_latest_block_number().await;

        // Get remote latest block number with timeout
        let remote_block_number = tokio::time::timeout(
            Duration::from_secs(30),
            async {
                let transport = transport_manager.read();
                transport.get_latest_block_number().await
            }
        ).await
        .context("Timeout getting latest block number")??;

        log::debug!("Sync check: local={}, remote={}", local_block_number, remote_block_number);

        if remote_block_number > local_block_number {
            let blocks_to_sync = remote_block_number - local_block_number;

            state_manager.update_sync_status(SyncStatus::Syncing {
                current: local_block_number,
                highest: remote_block_number,
                peers: 1, // Simplified
            }).await;

            // Sync missing blocks with reasonable batch size
            let max_batch_size = 50; // Prevent overwhelming the system
            let batch_size = blocks_to_sync.min(max_batch_size);

            let sync_result = Self::sync_blocks(
                state_manager,
                transport_manager,
                local_block_number + 1,
                local_block_number + batch_size,
            ).await;

            match sync_result {
                Ok(synced_count) => {
                    if synced_count > 0 {
                        log::info!("Synced {} blocks (from {} to {})", 
                                  synced_count, local_block_number + 1, local_block_number + synced_count);
                    }

                    // Update sync status
                    if remote_block_number <= local_block_number + synced_count {
                        state_manager.update_sync_status(SyncStatus::Synced).await;
                    }

                    // Update gas price periodically
                    if let Ok(gas_price) = {
                        let transport = transport_manager.read();
                        transport.get_gas_price().await
                    } {
                        state_manager.update_gas_price(gas_price).await;
                    }

                    Ok(synced_count)
                }
                Err(e) => {
                    log::error!("Failed to sync blocks: {}", e);
                    state_manager.update_sync_status(SyncStatus::Failed {
                        error: e.to_string(),
                        retry_count: 0,
                    }).await;
                    Err(e)
                }
            }
        } else {
            state_manager.update_sync_status(SyncStatus::Synced).await;

            // Still update gas price even when synced
            if let Ok(gas_price) = {
                let transport = transport_manager.read();
                transport.get_gas_price().await
            } {
                state_manager.update_gas_price(gas_price).await;
            }

            Ok(0)
        }
    }

    async fn sync_blocks(
        state_manager: &Arc<StateManager>,
        transport_manager: &Arc<RwLock<TransportManager>>,
        from_block: u64,
        to_block: u64,
    ) -> Result<u64> {
        let mut blocks = Vec::new();
        let mut synced_count = 0;

        for block_number in from_block..=to_block {
            let block_info = tokio::time::timeout(
                Duration::from_secs(10),
                async {
                    let transport = transport_manager.read();
                    transport.get_block_by_number(block_number).await
                }
            ).await
            .context(format!("Timeout getting block {}", block_number))??;

            if let Some(block) = block_info {
                blocks.push(block);
                synced_count += 1;

                // Batch update every 10 blocks for better performance
                if blocks.len() >= 10 {
                    state_manager.batch_update_blocks(blocks.clone()).await
                        .context("Failed to batch update blocks")?;
                    blocks.clear();
                }
            } else {
                log::warn!("Block {} not found during sync", block_number);
                break; // Stop syncing if we hit a missing block
            }
        }

        // Update remaining blocks
        if !blocks.is_empty() {
            state_manager.batch_update_blocks(blocks).await
                .context("Failed to update remaining blocks")?;
        }

        Ok(synced_count)
    }

    pub async fn force_sync(&self, from_block: Option<u64>, to_block: Option<u64>) -> Result<()> {
        if !self.is_running() {
            return Err(anyhow!("Sync engine not running"));
        }

        let from = from_block.unwrap_or(0);
        let to = to_block.unwrap_or({
            let transport = self.transport_manager.read();
            transport.get_latest_block_number().await
                .context("Failed to get latest block number for force sync")?
        });

        log::info!("Force syncing blocks {} to {}", from, to);

        let synced_count = Self::sync_blocks(&self.state_manager, &self.transport_manager, from, to).await
            .context("Force sync failed")?;

        log::info!("Force sync completed: {} blocks", synced_count);
        *self.last_sync.write() = Instant::now();

        Ok(())
    }

    pub fn get_sync_status(&self) -> SyncStatus {
        self.state_manager.get_sync_status()
    }

    pub fn get_last_sync_time(&self) -> Instant {
        *self.last_sync.read()
    }

    pub fn get_sync_stats(&self) -> SyncStats {
        self.sync_stats.read().clone()
    }
}
