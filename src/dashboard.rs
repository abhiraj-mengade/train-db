use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use warp::Filter;
use warp::Reply;

use crate::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStats {
    pub peer_id: String,
    pub listen_addresses: Vec<String>,
    pub connected_peers: Vec<String>,
    pub total_keys: usize,
    pub bluetooth_enabled: bool,
    pub uptime_seconds: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub last_activity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub transport: String,
    pub last_seen: String,
    pub is_connected: bool,
}

#[derive(Debug, Clone)]
pub struct NetworkState {
    pub connected_peers: Vec<PeerInfo>,
    pub listen_addresses: Vec<String>,
    pub peer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub node_stats: NodeStats,
    pub peers: Vec<PeerInfo>,
    pub recent_activity: Vec<String>,
}

pub struct Dashboard {
    storage: Arc<Mutex<Box<dyn Storage>>>,
    start_time: std::time::Instant,
    messages_sent: Arc<Mutex<u64>>,
    messages_received: Arc<Mutex<u64>>,
    recent_activity: Arc<Mutex<Vec<String>>>,
    network_state: Arc<Mutex<NetworkState>>,
}

impl Dashboard {
    pub fn new(storage: Arc<Mutex<Box<dyn Storage>>>) -> Self {
        Self {
            storage,
            start_time: std::time::Instant::now(),
            messages_sent: Arc::new(Mutex::new(0)),
            messages_received: Arc::new(Mutex::new(0)),
            recent_activity: Arc::new(Mutex::new(Vec::new())),
            network_state: Arc::new(Mutex::new(NetworkState {
                connected_peers: Vec::new(),
                listen_addresses: Vec::new(),
                peer_id: String::new(),
            })),
        }
    }

    pub fn increment_messages_sent(&self) {
        if let Ok(mut count) = self.messages_sent.try_lock() {
            *count += 1;
        }
    }

    pub fn increment_messages_received(&self) {
        if let Ok(mut count) = self.messages_received.try_lock() {
            *count += 1;
        }
    }

    pub fn add_activity(&self, activity: String) {
        if let Ok(mut activities) = self.recent_activity.try_lock() {
            activities.push(activity);
            // Keep only last 50 activities
            if activities.len() > 50 {
                activities.remove(0);
            }
        }
    }

    pub fn update_network_state(&self, peer_id: String, listen_addresses: Vec<String>) {
        if let Ok(mut state) = self.network_state.try_lock() {
            state.peer_id = peer_id;
            state.listen_addresses = listen_addresses;
        }
    }

    pub fn add_peer(&self, peer: PeerInfo) {
        if let Ok(mut state) = self.network_state.try_lock() {
            // Remove existing peer if present
            state.connected_peers.retain(|p| p.peer_id != peer.peer_id);
            // Add new peer
            state.connected_peers.push(peer);
        }
    }

    pub fn remove_peer(&self, peer_id: &str) {
        if let Ok(mut state) = self.network_state.try_lock() {
            state.connected_peers.retain(|p| p.peer_id != peer_id);
        }
    }

    async fn get_node_stats(&self, peer_id: String, listen_addresses: Vec<String>) -> Result<NodeStats> {
        let total_keys = {
            let storage = self.storage.lock().await;
            storage.list_keys()?.len()
        };

        let uptime = self.start_time.elapsed().as_secs();
        let messages_sent = *self.messages_sent.lock().await;
        let messages_received = *self.messages_received.lock().await;

        // Get actual connected peers from network state
        let connected_peers = {
            let state = self.network_state.lock().await;
            state.connected_peers.iter().map(|p| p.peer_id.clone()).collect()
        };

        Ok(NodeStats {
            peer_id,
            listen_addresses,
            connected_peers,
            total_keys,
            bluetooth_enabled: cfg!(feature = "bluetooth"),
            uptime_seconds: uptime,
            messages_sent,
            messages_received,
            last_activity: chrono::Utc::now().to_rfc3339(),
        })
    }

    async fn get_dashboard_data(&self, peer_id: String, listen_addresses: Vec<String>) -> Result<DashboardData> {
        let node_stats = self.get_node_stats(peer_id, listen_addresses).await?;
        
        // Get actual peers from the network state
        let peers = {
            let network_state = self.network_state.lock().await;
            network_state.connected_peers.clone()
        };
        
        let recent_activity = {
            let activities = self.recent_activity.lock().await;
            activities.clone()
        };

        Ok(DashboardData {
            node_stats,
            peers,
            recent_activity,
        })
    }

    async fn get_database_keys(&self) -> Result<serde_json::Value> {
        let storage = self.storage.lock().await;
        let keys = storage.list_keys()?;
        
        let mut key_values = Vec::new();
        for key in keys {
            if let Ok(value) = storage.get(&key) {
                key_values.push(serde_json::json!({
                    "key": key,
                    "value": value
                }));
            }
        }
        
        Ok(serde_json::json!({
            "keys": key_values,
            "total_count": key_values.len()
        }))
    }

    pub fn create_routes(
        self: Arc<Self>,
        peer_id: String,
        listen_addresses: Vec<String>,
    ) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        let dashboard1 = self.clone();
        let dashboard2 = self.clone();
        let dashboard3 = self.clone();
        let peer_id_clone1 = peer_id.clone();
        let peer_id_clone2 = peer_id.clone();
        let listen_addresses_clone1 = listen_addresses.clone();
        let listen_addresses_clone2 = listen_addresses.clone();

        // Serve the main dashboard HTML
        let dashboard_html = warp::path::end()
            .map(|| {
                warp::reply::html(include_str!("../dashboard/index.html"))
            });

        // Dashboard data endpoint
        let dashboard_data = warp::path("api")
            .and(warp::path("dashboard"))
            .and(warp::get())
            .and_then(move || {
                let dashboard = dashboard1.clone();
                let peer_id = peer_id_clone1.clone();
                let listen_addresses = listen_addresses_clone1.clone();
                async move {
                    match dashboard.get_dashboard_data(peer_id, listen_addresses).await {
                        Ok(data) => Ok::<_, warp::Rejection>(warp::reply::json(&data)),
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                            "error": e.to_string()
                        }))),
                    }
                }
            });

        // Node stats endpoint
        let node_stats = warp::path("api")
            .and(warp::path("stats"))
            .and(warp::get())
            .and_then(move || {
                let dashboard = dashboard2.clone();
                let peer_id = peer_id_clone2.clone();
                let listen_addresses = listen_addresses_clone2.clone();
                async move {
                    match dashboard.get_node_stats(peer_id, listen_addresses).await {
                        Ok(stats) => Ok::<_, warp::Rejection>(warp::reply::json(&stats)),
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                            "error": e.to_string()
                        }))),
                    }
                }
            });

        // Database keys endpoint
        let database_keys = warp::path("api")
            .and(warp::path("keys"))
            .and(warp::get())
            .and_then(move || {
                let dashboard = dashboard3.clone();
                async move {
                    match dashboard.get_database_keys().await {
                        Ok(keys) => Ok::<_, warp::Rejection>(warp::reply::json(&keys)),
                        Err(e) => Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                            "error": e.to_string()
                        }))),
                    }
                }
            });

        // Combine all routes
        dashboard_html
            .or(dashboard_data)
            .or(node_stats)
            .or(database_keys)
    }
}
