pub mod http_transport;
pub mod bluetooth_transport;

use async_trait::async_trait;
use anyhow::Result;
use crate::blockchain_state::{BlockInfo, TransactionInfo};
use ethereum_types::{H256, U256, Address};
use std::time::Duration;
use parking_lot::Mutex; // Better mutex to prevent deadlocks

#[async_trait]
pub trait Transport: Send + Sync {
    async fn initialize(&mut self) -> Result<()>;
    async fn is_available(&self) -> bool;
    async fn get_latest_block_number(&self) -> Result<u64>;
    async fn get_block_by_number(&self, number: u64) -> Result<Option<BlockInfo>>;
    async fn get_block_by_hash(&self, hash: &H256) -> Result<Option<BlockInfo>>;
    async fn get_transaction(&self, hash: &H256) -> Result<Option<TransactionInfo>>;
    async fn get_balance(&self, address: &Address) -> Result<U256>;
    async fn get_transaction_count(&self, address: &Address) -> Result<U256>;
    async fn send_raw_transaction(&self, tx_data: &[u8]) -> Result<H256>;
    async fn call_contract(&self, to: &Address, data: &[u8]) -> Result<Vec<u8>>;
    async fn estimate_gas(&self, to: Option<&Address>, data: &[u8]) -> Result<U256>;
    async fn get_gas_price(&self) -> Result<U256>;
    async fn get_health_score(&self) -> f32; // 0.0 = unhealthy, 1.0 = perfect
    async fn shutdown(&mut self) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportType {
    Http,
    Bluetooth,
}

#[derive(Debug, Clone)]
struct TransportHealth {
    transport_type: TransportType,
    last_success: Option<std::time::Instant>,
    failure_count: u32,
    total_requests: u64,
    successful_requests: u64,
    avg_response_time: Duration,
}

impl TransportHealth {
    fn new(transport_type: TransportType) -> Self {
        Self {
            transport_type,
            last_success: None,
            failure_count: 0,
            total_requests: 0,
            successful_requests: 0,
            avg_response_time: Duration::from_millis(1000),
        }
    }

    fn record_success(&mut self, response_time: Duration) {
        self.last_success = Some(std::time::Instant::now());
        self.failure_count = 0;
        self.total_requests += 1;
        self.successful_requests += 1;

        // Update average response time (simple moving average)
        self.avg_response_time = Duration::from_millis(
            (self.avg_response_time.as_millis() as u64 + response_time.as_millis() as u64) / 2
        );
    }

    fn record_failure(&mut self) {
        self.failure_count += 1;
        self.total_requests += 1;
    }

    fn health_score(&self) -> f32 {
        if self.total_requests == 0 {
            return 0.5; // Unknown
        }

        let success_rate = self.successful_requests as f32 / self.total_requests as f32;

        // Factor in recent failures
        let failure_penalty = (self.failure_count as f32 * 0.1).min(0.5);

        // Factor in response time (penalty for slow responses)
        let response_penalty = if self.avg_response_time > Duration::from_secs(5) {
            0.2
        } else if self.avg_response_time > Duration::from_secs(2) {
            0.1
        } else {
            0.0
        };

        // Factor in time since last success
        let staleness_penalty = if let Some(last_success) = self.last_success {
            if last_success.elapsed() > Duration::from_secs(60) {
                0.3
            } else if last_success.elapsed() > Duration::from_secs(30) {
                0.1
            } else {
                0.0
            }
        } else {
            0.4 // Never succeeded
        };

        (success_rate - failure_penalty - response_penalty - staleness_penalty).max(0.0).min(1.0)
    }
}

pub struct TransportManager {
    primary_transport: Box<dyn Transport>,
    fallback_transport: Option<Box<dyn Transport>>,
    current_transport: Mutex<TransportType>, // Thread-safe current transport
    primary_health: Mutex<TransportHealth>,
    fallback_health: Mutex<TransportHealth>,
    switch_threshold: f32,
    last_health_check: Mutex<std::time::Instant>,
}

impl TransportManager {
    pub fn new(
        primary: Box<dyn Transport>,
        fallback: Option<Box<dyn Transport>>,
    ) -> Self {
        Self {
            primary_transport: primary,
            fallback_transport: fallback,
            current_transport: Mutex::new(TransportType::Http),
            primary_health: Mutex::new(TransportHealth::new(TransportType::Http)),
            fallback_health: Mutex::new(TransportHealth::new(TransportType::Bluetooth)),
            switch_threshold: 0.3, // Switch when health drops below 30%
            last_health_check: Mutex::new(std::time::Instant::now()),
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        // Initialize primary transport
        if let Err(e) = self.primary_transport.initialize().await {
            log::error!("Failed to initialize primary transport: {}", e);
            self.primary_health.lock().record_failure();
        } else {
            log::info!("Primary transport initialized successfully");
        }

        // Initialize fallback transport if available
        if let Some(ref mut fallback) = self.fallback_transport {
            match fallback.initialize().await {
                Ok(_) => {
                    log::info!("Fallback transport initialized successfully");
                }
                Err(e) => {
                    log::warn!("Failed to initialize fallback transport: {}", e);
                    self.fallback_health.lock().record_failure();
                }
            }
        }

        Ok(())
    }

    async fn should_switch_transport(&self) -> (bool, TransportType) {
        // Rate limit health checks
        {
            let mut last_check = self.last_health_check.lock();
            if last_check.elapsed() < Duration::from_secs(10) {
                return (false, *self.current_transport.lock());
            }
            *last_check = std::time::Instant::now();
        }

        let current = *self.current_transport.lock();
        let primary_health = self.primary_health.lock().health_score();
        let fallback_health = self.fallback_health.lock().health_score();

        match current {
            TransportType::Http => {
                // Switch to fallback if primary is unhealthy and fallback is better
                if primary_health < self.switch_threshold && 
                   fallback_health > primary_health + 0.2 && 
                   self.fallback_transport.is_some() {
                    log::warn!("Switching to fallback transport (Primary health: {:.2}, Fallback health: {:.2})", 
                               primary_health, fallback_health);
                    return (true, TransportType::Bluetooth);
                }
            }
            TransportType::Bluetooth => {
                // Switch back to primary if it's healthier
                if primary_health > fallback_health + 0.2 && 
                   primary_health > self.switch_threshold + 0.1 {
                    log::info!("Switching back to primary transport (Primary health: {:.2}, Fallback health: {:.2})", 
                               primary_health, fallback_health);
                    return (true, TransportType::Http);
                }
            }
        }

        (false, current)
    }

    async fn get_active_transport(&self) -> (&dyn Transport, TransportType) {
        let current = *self.current_transport.lock();
        match current {
            TransportType::Http => (&*self.primary_transport, current),
            TransportType::Bluetooth => {
                if let Some(ref fallback) = self.fallback_transport {
                    (&**fallback, current)
                } else {
                    (&*self.primary_transport, TransportType::Http)
                }
            }
        }
    }

    async fn execute_with_fallback<T, F, Fut>(&self, operation_name: &str, operation: F) -> Result<T>
    where
        F: Fn(&dyn Transport) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        // Check if we should switch transport
        let (should_switch, new_transport) = self.should_switch_transport().await;
        if should_switch {
            *self.current_transport.lock() = new_transport;
        }

        let start_time = std::time::Instant::now();
        let (transport, transport_type) = self.get_active_transport().await;

        match operation(transport).await {
            Ok(result) => {
                let response_time = start_time.elapsed();

                // Record success
                match transport_type {
                    TransportType::Http => {
                        self.primary_health.lock().record_success(response_time);
                    }
                    TransportType::Bluetooth => {
                        self.fallback_health.lock().record_success(response_time);
                    }
                }

                log::debug!("{} succeeded via {:?} in {:?}", operation_name, transport_type, response_time);
                Ok(result)
            }
            Err(e) => {
                // Record failure
                match transport_type {
                    TransportType::Http => {
                        self.primary_health.lock().record_failure();
                    }
                    TransportType::Bluetooth => {
                        self.fallback_health.lock().record_failure();
                    }
                }

                log::debug!("{} failed via {:?}: {}", operation_name, transport_type, e);

                // Try fallback if we're using primary and fallback is available
                if transport_type == TransportType::Http && self.fallback_transport.is_some() {
                    log::info!("Attempting {} via fallback transport", operation_name);

                    *self.current_transport.lock() = TransportType::Bluetooth;
                    let (fallback_transport, _) = self.get_active_transport().await;

                    match operation(fallback_transport).await {
                        Ok(result) => {
                            let response_time = start_time.elapsed();
                            self.fallback_health.lock().record_success(response_time);
                            log::info!("{} succeeded via fallback transport", operation_name);
                            Ok(result)
                        }
                        Err(fallback_err) => {
                            self.fallback_health.lock().record_failure();
                            log::error!("{} failed on both transports. Primary: {}, Fallback: {}", 
                                       operation_name, e, fallback_err);
                            Err(e) // Return original error
                        }
                    }
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn get_transport_stats(&self) -> TransportStats {
        let primary_health = self.primary_health.lock();
        let fallback_health = self.fallback_health.lock();

        TransportStats {
            current_transport: *self.current_transport.lock(),
            primary_health_score: primary_health.health_score(),
            primary_requests: primary_health.total_requests,
            primary_success_rate: if primary_health.total_requests > 0 {
                primary_health.successful_requests as f32 / primary_health.total_requests as f32
            } else {
                0.0
            },
            fallback_health_score: fallback_health.health_score(),
            fallback_requests: fallback_health.total_requests,
            fallback_success_rate: if fallback_health.total_requests > 0 {
                fallback_health.successful_requests as f32 / fallback_health.total_requests as f32
            } else {
                0.0
            },
            has_fallback: self.fallback_transport.is_some(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransportStats {
    pub current_transport: TransportType,
    pub primary_health_score: f32,
    pub primary_requests: u64,
    pub primary_success_rate: f32,
    pub fallback_health_score: f32,
    pub fallback_requests: u64,
    pub fallback_success_rate: f32,
    pub has_fallback: bool,
}

#[async_trait]
impl Transport for TransportManager {
    async fn initialize(&mut self) -> Result<()> {
        TransportManager::initialize(self).await
    }

    async fn is_available(&self) -> bool {
        let (transport, _) = self.get_active_transport().await;
        transport.is_available().await
    }

    async fn get_latest_block_number(&self) -> Result<u64> {
        self.execute_with_fallback("get_latest_block_number", |transport| {
            transport.get_latest_block_number()
        }).await
    }

    async fn get_block_by_number(&self, number: u64) -> Result<Option<BlockInfo>> {
        self.execute_with_fallback("get_block_by_number", |transport| {
            transport.get_block_by_number(number)
        }).await
    }

    async fn get_block_by_hash(&self, hash: &H256) -> Result<Option<BlockInfo>> {
        let hash_copy = *hash;
        self.execute_with_fallback("get_block_by_hash", move |transport| {
            transport.get_block_by_hash(&hash_copy)
        }).await
    }

    async fn get_transaction(&self, hash: &H256) -> Result<Option<TransactionInfo>> {
        let hash_copy = *hash;
        self.execute_with_fallback("get_transaction", move |transport| {
            transport.get_transaction(&hash_copy)
        }).await
    }

    async fn get_balance(&self, address: &Address) -> Result<U256> {
        let address_copy = *address;
        self.execute_with_fallback("get_balance", move |transport| {
            transport.get_balance(&address_copy)
        }).await
    }

    async fn get_transaction_count(&self, address: &Address) -> Result<U256> {
        let address_copy = *address;
        self.execute_with_fallback("get_transaction_count", move |transport| {
            transport.get_transaction_count(&address_copy)
        }).await
    }

    async fn send_raw_transaction(&self, tx_data: &[u8]) -> Result<H256> {
        let tx_data_copy = tx_data.to_vec();
        self.execute_with_fallback("send_raw_transaction", move |transport| {
            transport.send_raw_transaction(&tx_data_copy)
        }).await
    }

    async fn call_contract(&self, to: &Address, data: &[u8]) -> Result<Vec<u8>> {
        let to_copy = *to;
        let data_copy = data.to_vec();
        self.execute_with_fallback("call_contract", move |transport| {
            transport.call_contract(&to_copy, &data_copy)
        }).await
    }

    async fn estimate_gas(&self, to: Option<&Address>, data: &[u8]) -> Result<U256> {
        let to_copy = to.copied();
        let data_copy = data.to_vec();
        self.execute_with_fallback("estimate_gas", move |transport| {
            transport.estimate_gas(to_copy.as_ref(), &data_copy)
        }).await
    }

    async fn get_gas_price(&self) -> Result<U256> {
        self.execute_with_fallback("get_gas_price", |transport| {
            transport.get_gas_price()
        }).await
    }

    async fn get_health_score(&self) -> f32 {
        let (transport, transport_type) = self.get_active_transport().await;
        let base_score = transport.get_health_score().await;

        // Adjust based on our health tracking
        let health_adjustment = match transport_type {
            TransportType::Http => self.primary_health.lock().health_score(),
            TransportType::Bluetooth => self.fallback_health.lock().health_score(),
        };

        (base_score + health_adjustment) / 2.0
    }

    async fn shutdown(&mut self) -> Result<()> {
        log::info!("Shutting down transport manager");

        if let Err(e) = self.primary_transport.shutdown().await {
            log::error!("Error shutting down primary transport: {}", e);
        }

        if let Some(ref mut fallback) = self.fallback_transport {
            if let Err(e) = fallback.shutdown().await {
                log::error!("Error shutting down fallback transport: {}", e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_health_scoring() {
        let mut health = TransportHealth::new(TransportType::Http);

        // Test initial state
        assert_eq!(health.health_score(), 0.5);

        // Test after success
        health.record_success(Duration::from_millis(100));
        assert!(health.health_score() > 0.8);

        // Test after failures
        for _ in 0..5 {
            health.record_failure();
        }
        assert!(health.health_score() < 0.5);
    }
}
