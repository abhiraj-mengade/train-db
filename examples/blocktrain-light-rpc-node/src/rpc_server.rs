use std::sync::Arc;
use anyhow::Result;
use parking_lot::RwLock;
use jsonrpc_core::{IoHandler, Params, Value, Error as JsonRpcError, ErrorCode};
use jsonrpc_http_server::{ServerBuilder, Server, AccessControlAllowOrigin, DomainsValidation};
use serde_json::json;
use crate::config::Config;
use crate::blockchain_state::StateManager;
use crate::sync_engine::SyncEngine;
use crate::transport::TransportManager;
use crate::utils;

pub struct RpcServer {
    config: Config,
    state_manager: Arc<StateManager>,
    sync_engine: Arc<SyncEngine>,
    transport_manager: Arc<RwLock<TransportManager>>,
    server: Option<Server>,
}

impl RpcServer {
    pub fn new(
        config: Config,
        state_manager: Arc<StateManager>,
        sync_engine: Arc<SyncEngine>,
        transport_manager: Arc<RwLock<TransportManager>>,
    ) -> Self {
        Self {
            config,
            state_manager,
            sync_engine,
            transport_manager,
            server: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let mut io = IoHandler::new();

        let state_manager = self.state_manager.clone();
        let sync_engine = self.sync_engine.clone();

        // eth_blockNumber
        {
            let state_manager = state_manager.clone();
            io.add_sync_method("eth_blockNumber", move |_: Params| {
                let block_number = futures::executor::block_on(async {
                    state_manager.get_latest_block_number().await
                });
                Ok(json!(utils::format_u64(block_number)))
            });
        }

        // eth_getBalance
        {
            let state_manager = state_manager.clone();
            io.add_sync_method("eth_getBalance", move |params: Params| {
                let params: Vec<Value> = params.parse()?;
                if params.len() < 1 {
                    return Err(JsonRpcError::new(ErrorCode::InvalidParams));
                }

                let address_str = params[0].as_str()
                    .ok_or_else(|| JsonRpcError::new(ErrorCode::InvalidParams))?;

                match utils::parse_address(address_str) {
                    Ok(address) => {
                        let balance = futures::executor::block_on(async {
                            state_manager.get_account_balance(&address).await
                        });
                        Ok(json!(utils::format_u256(&balance)))
                    }
                    Err(_) => Err(JsonRpcError::new(ErrorCode::InvalidParams)),
                }
            });
        }

        // eth_getTransactionCount
        {
            let state_manager = state_manager.clone();
            io.add_sync_method("eth_getTransactionCount", move |params: Params| {
                let params: Vec<Value> = params.parse()?;
                if params.len() < 1 {
                    return Err(JsonRpcError::new(ErrorCode::InvalidParams));
                }

                let address_str = params[0].as_str()
                    .ok_or_else(|| JsonRpcError::new(ErrorCode::InvalidParams))?;

                match utils::parse_address(address_str) {
                    Ok(address) => {
                        let nonce = futures::executor::block_on(async {
                            state_manager.get_account_nonce(&address).await
                        });
                        Ok(json!(utils::format_u256(&nonce)))
                    }
                    Err(_) => Err(JsonRpcError::new(ErrorCode::InvalidParams)),
                }
            });
        }

        // net_version
        {
            let chain_id = self.config.blockchain.chain_id;
            io.add_sync_method("net_version", move |_: Params| {
                Ok(json!(chain_id.to_string()))
            });
        }

        // web3_clientVersion
        io.add_sync_method("web3_clientVersion", |_: Params| {
            Ok(json!("lightweight-rpc-node/0.1.0"))
        });

        // debug_syncStatus
        {
            let sync_engine = sync_engine.clone();
            io.add_sync_method("debug_syncStatus", move |_: Params| {
                let status = sync_engine.get_sync_status();
                let stats = sync_engine.get_sync_stats();

                Ok(json!({
                    "syncStatus": match status {
                        crate::blockchain_state::SyncStatus::NotSynced => "not_synced",
                        crate::blockchain_state::SyncStatus::Syncing { .. } => "syncing",
                        crate::blockchain_state::SyncStatus::Synced => "synced",
                        crate::blockchain_state::SyncStatus::Failed { .. } => "failed",
                    },
                    "totalBlocksSynced": stats.total_blocks_synced,
                    "successfulSyncs": stats.successful_syncs,
                    "syncErrors": stats.sync_errors,
                    "isRunning": sync_engine.is_running(),
                }))
            });
        }

        let cors = DomainsValidation::AllowOnly(vec![AccessControlAllowOrigin::Any]);

        let server = ServerBuilder::new(io)
            .cors(cors)
            .threads(2)
            .start_http(&self.config.rpc.listen_addr)?;

        log::info!("RPC server started on {}", self.config.rpc.listen_addr);
        self.server = Some(server);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(server) = self.server.take() {
            log::info!("Stopping RPC server");
            server.close();
        }
    }
}

impl Drop for RpcServer {
    fn drop(&mut self) {
        self.stop();
    }
}
