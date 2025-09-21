use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error, debug};
use libp2p::PeerId;

use crate::storage::Storage;
use crate::bluetooth::{BluetoothTransport, MultiTransport};

// Transport types for the mesh
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TransportType {
    Wifi,
    Bluetooth,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub transport: TransportType,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub is_connected: bool,
    pub connection_quality: f32, // 0.0 to 1.0
}

// Auto-mesh node that intelligently switches between transports
pub struct AutoMeshNode {
    storage: Arc<Mutex<Box<dyn Storage>>>,
    bluetooth_transport: Option<BluetoothTransport>,
    discovered_peers: Arc<Mutex<HashMap<PeerId, PeerInfo>>>,
    transport_health: Arc<Mutex<HashMap<TransportType, bool>>>,
    is_running: bool,
}

impl AutoMeshNode {
    pub fn new<S: Storage + 'static>(storage: S) -> Result<Self> {
        let bluetooth_transport = match BluetoothTransport::new() {
            Ok(bt) => {
                info!("Bluetooth transport initialized");
                Some(bt)
            }
            Err(e) => {
                warn!("Failed to initialize Bluetooth transport: {}", e);
                None
            }
        };

        let mut transport_health = HashMap::new();
        transport_health.insert(TransportType::Wifi, true); // Assume Wi-Fi is available
        transport_health.insert(TransportType::Bluetooth, bluetooth_transport.is_some());

        Ok(Self {
            storage: Arc::new(Mutex::new(Box::new(storage))),
            bluetooth_transport,
            discovered_peers: Arc::new(Mutex::new(HashMap::new())),
            transport_health: Arc::new(Mutex::new(transport_health)),
            is_running: false,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }

        info!("🚀 Starting Auto-Mesh Node");
        info!("   This node will automatically try multiple transports to find peers");
        info!("   and intelligently route messages through the best available path.");

        // Start Bluetooth transport if available
        if let Some(ref mut bt) = self.bluetooth_transport {
            match bt.start().await {
                Ok(()) => {
                    info!("✅ Bluetooth transport started");
                    self.transport_health.lock().await.insert(TransportType::Bluetooth, true);
                }
                Err(e) => {
                    warn!("❌ Bluetooth transport failed to start: {}", e);
                    self.transport_health.lock().await.insert(TransportType::Bluetooth, false);
                }
            }
        }

        self.is_running = true;
        info!("🎯 Auto-Mesh Node is now running and discovering peers...");

        // Start discovery tasks
        self.start_discovery_tasks().await?;

        Ok(())
    }

    async fn start_discovery_tasks(&mut self) -> Result<()> {
        let discovered_peers = self.discovered_peers.clone();
        let transport_health = self.transport_health.clone();

        // Start Wi-Fi discovery task (simulated for demo)
        let wifi_peers = discovered_peers.clone();
        let wifi_health = transport_health.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                
                // Simulate Wi-Fi discovery
                if *wifi_health.lock().await.get(&TransportType::Wifi).unwrap_or(&false) {
                    debug!("🔍 Scanning for Wi-Fi peers...");
                    
                    // Simulate finding peers over Wi-Fi
                    let wifi_peer_count = Self::simulate_wifi_discovery().await;
                    if wifi_peer_count > 0 {
                        info!("📡 Found {} peers over Wi-Fi", wifi_peer_count);
                        Self::update_peer_info(&wifi_peers, TransportType::Wifi, wifi_peer_count).await;
                    } else {
                        warn!("📡 No peers found over Wi-Fi - transport may be failing");
                        wifi_health.lock().await.insert(TransportType::Wifi, false);
                    }
                } else {
                    debug!("📡 Wi-Fi transport is down, skipping discovery");
                }
            }
        });

        // Start Bluetooth discovery task
        if let Some(ref mut bt) = self.bluetooth_transport {
            let bt_peers = discovered_peers.clone();
            let bt_health = transport_health.clone();
            
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    
                    if *bt_health.lock().await.get(&TransportType::Bluetooth).unwrap_or(&false) {
                        debug!("🔵 Scanning for Bluetooth peers...");
                        
                        // Note: In a real implementation, we'd need to handle the borrow checker
                        // For now, we'll simulate Bluetooth discovery
                        let simulated_peers = Self::simulate_bluetooth_discovery().await;
                        if simulated_peers > 0 {
                            info!("🔵 Found {} peers over Bluetooth", simulated_peers);
                            Self::update_peer_info(&bt_peers, TransportType::Bluetooth, simulated_peers).await;
                        } else {
                            debug!("🔵 No Bluetooth peers found");
                        }
                    } else {
                        debug!("🔵 Bluetooth transport is down, skipping discovery");
                    }
                }
            });
        }

        Ok(())
    }

    // Simulate Wi-Fi discovery for demo purposes
    async fn simulate_wifi_discovery() -> usize {
        // Simulate network conditions
        let random = fastrand::f32();
        if random > 0.7 {
            // 30% chance of finding peers over Wi-Fi
            fastrand::usize(0..3)
        } else {
            0
        }
    }

    // Simulate Bluetooth discovery for demo purposes
    async fn simulate_bluetooth_discovery() -> usize {
        // Simulate Bluetooth discovery (more reliable than Wi-Fi in some scenarios)
        let random = fastrand::f32();
        if random > 0.5 {
            // 50% chance of finding peers over Bluetooth
            fastrand::usize(0..2)
        } else {
            0
        }
    }

    async fn update_peer_info(
        peers: &Arc<Mutex<HashMap<PeerId, PeerInfo>>>,
        transport: TransportType,
        count: usize,
    ) {
        let mut peer_map = peers.lock().await;
        
        for _i in 0..count {
            let peer_id = PeerId::random();
            let peer_info = PeerInfo {
                peer_id,
                transport: transport.clone(),
                last_seen: chrono::Utc::now(),
                is_connected: true,
                connection_quality: 0.8 + (fastrand::f32() * 0.2), // 0.8-1.0
            };
            peer_map.insert(peer_id, peer_info);
        }
    }

    async fn update_bluetooth_peers(
        peers: &Arc<Mutex<HashMap<PeerId, PeerInfo>>>,
        discovered_peers: Vec<PeerId>,
    ) {
        let mut peer_map = peers.lock().await;
        
        for peer_id in discovered_peers {
            let peer_info = PeerInfo {
                peer_id,
                transport: TransportType::Bluetooth,
                last_seen: chrono::Utc::now(),
                is_connected: true,
                connection_quality: 0.6 + (fastrand::f32() * 0.3), // 0.6-0.9 (Bluetooth is typically lower quality)
            };
            peer_map.insert(peer_id, peer_info);
        }
    }

    pub async fn get_network_status(&self) -> NetworkStatus {
        let peers = self.discovered_peers.lock().await;
        let health = self.transport_health.lock().await;
        
        let total_peers = peers.len();
        let wifi_peers = peers.values().filter(|p| p.transport == TransportType::Wifi).count();
        let bluetooth_peers = peers.values().filter(|p| p.transport == TransportType::Bluetooth).count();
        
        let wifi_healthy = *health.get(&TransportType::Wifi).unwrap_or(&false);
        let bluetooth_healthy = *health.get(&TransportType::Bluetooth).unwrap_or(&false);
        
        NetworkStatus {
            total_peers,
            wifi_peers,
            bluetooth_peers,
            wifi_healthy,
            bluetooth_healthy,
            mesh_quality: self.calculate_mesh_quality(&peers),
        }
    }

    fn calculate_mesh_quality(&self, peers: &HashMap<PeerId, PeerInfo>) -> f32 {
        if peers.is_empty() {
            return 0.0;
        }
        
        let avg_quality: f32 = peers.values()
            .map(|p| p.connection_quality)
            .sum::<f32>() / peers.len() as f32;
        
        let transport_diversity = if peers.values().any(|p| p.transport == TransportType::Wifi) 
            && peers.values().any(|p| p.transport == TransportType::Bluetooth) {
            1.0 // Both transports have peers
        } else if !peers.is_empty() {
            0.5 // Only one transport has peers
        } else {
            0.0 // No peers
        };
        
        (avg_quality + transport_diversity) / 2.0
    }

    pub async fn send_message(&mut self, peer_id: PeerId, message: Vec<u8>) -> Result<()> {
        let peers = self.discovered_peers.lock().await;
        
        if let Some(peer_info) = peers.get(&peer_id) {
            info!("📤 Sending message to peer {} via {:?}", peer_id, peer_info.transport);
            
            match peer_info.transport {
                TransportType::Wifi => {
                    info!("📡 Routing via Wi-Fi (primary transport)");
                    // In a real implementation, this would use the Wi-Fi transport
                    self.send_via_wifi(peer_id, message).await?;
                }
                TransportType::Bluetooth => {
                    info!("🔵 Routing via Bluetooth (backup transport)");
                    if let Some(ref mut bt) = self.bluetooth_transport {
                        bt.send_message(peer_id, message).await?;
                    }
                }
            }
        } else {
            warn!("❌ Peer {} not found in discovered peers", peer_id);
            return Err(anyhow::anyhow!("Peer not found"));
        }
        
        Ok(())
    }

    async fn send_via_wifi(&self, _peer_id: PeerId, _message: Vec<u8>) -> Result<()> {
        // Simulate Wi-Fi message sending
        info!("📡 Message sent via Wi-Fi transport");
        Ok(())
    }

    pub async fn run_mesh_demo(&mut self, duration_seconds: u64) -> Result<()> {
        info!("🎬 Starting Auto-Mesh Demo for {} seconds", duration_seconds);
        info!("   Watch how the node automatically discovers peers and switches transports!");
        
        let start_time = std::time::Instant::now();
        
        while start_time.elapsed().as_secs() < duration_seconds {
            let status = self.get_network_status().await;
            
            info!("📊 Mesh Status:");
            info!("   Total Peers: {}", status.total_peers);
            info!("   Wi-Fi Peers: {} (healthy: {})", status.wifi_peers, status.wifi_healthy);
            info!("   Bluetooth Peers: {} (healthy: {})", status.bluetooth_peers, status.bluetooth_healthy);
            info!("   Mesh Quality: {:.2}/1.0", status.mesh_quality);
            
            if status.mesh_quality > 0.7 {
                info!("✅ Excellent mesh connectivity!");
            } else if status.mesh_quality > 0.4 {
                info!("⚠️  Moderate mesh connectivity");
            } else {
                info!("❌ Poor mesh connectivity - trying alternative transports...");
            }
            
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
        
        info!("🎬 Auto-Mesh Demo completed!");
        Ok(())
    }
}

#[derive(Debug)]
pub struct NetworkStatus {
    pub total_peers: usize,
    pub wifi_peers: usize,
    pub bluetooth_peers: usize,
    pub wifi_healthy: bool,
    pub bluetooth_healthy: bool,
    pub mesh_quality: f32,
}
