use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
}

impl Dashboard {
    pub fn new(storage: Arc<Mutex<Box<dyn Storage>>>) -> Self {
        Self {
            storage,
            start_time: std::time::Instant::now(),
            messages_sent: Arc::new(Mutex::new(0)),
            messages_received: Arc::new(Mutex::new(0)),
            recent_activity: Arc::new(Mutex::new(Vec::new())),
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

    async fn get_node_stats(&self, peer_id: String, listen_addresses: Vec<String>) -> Result<NodeStats> {
        let total_keys = {
            let storage = self.storage.lock().await;
            storage.list_keys()?.len()
        };

        let uptime = self.start_time.elapsed().as_secs();
        let messages_sent = *self.messages_sent.lock().await;
        let messages_received = *self.messages_received.lock().await;

        Ok(NodeStats {
            peer_id,
            listen_addresses,
            connected_peers: vec![], // TODO: Get from swarm
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
        
        // Generate some demo peers for demonstration
        let peers = vec![
            PeerInfo {
                peer_id: "12D3KooWPeer1Demo123456789".to_string(),
                transport: "Wi-Fi".to_string(),
                last_seen: chrono::Utc::now().to_rfc3339(),
                is_connected: true,
            },
            PeerInfo {
                peer_id: "12D3KooWPeer2Demo987654321".to_string(),
                transport: "Bluetooth".to_string(),
                last_seen: chrono::Utc::now().to_rfc3339(),
                is_connected: true,
            },
        ];
        
        let recent_activity = {
            let mut activities = self.recent_activity.lock().await;
            // Add some demo activities if empty
            if activities.is_empty() {
                activities.push("Node started successfully".to_string());
                activities.push("Bluetooth transport initialized".to_string());
                activities.push("Listening on TCP port 8082".to_string());
                activities.push("Peer discovery started".to_string());
            }
            activities.clone()
        };

        Ok(DashboardData {
            node_stats,
            peers,
            recent_activity,
        })
    }

    pub fn create_routes(
        self: Arc<Self>,
        peer_id: String,
        listen_addresses: Vec<String>,
    ) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
        let dashboard = self.clone();
        let peer_id_clone = peer_id.clone();
        let listen_addresses_clone = listen_addresses.clone();

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
                let dashboard = dashboard.clone();
                let peer_id = peer_id_clone.clone();
                let listen_addresses = listen_addresses_clone.clone();
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
                let dashboard = self.clone();
                let peer_id = peer_id.clone();
                let listen_addresses = listen_addresses.clone();
                async move {
                    match dashboard.get_node_stats(peer_id, listen_addresses).await {
                        Ok(stats) => Ok::<_, warp::Rejection>(warp::reply::json(&stats)),
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
    }
}
