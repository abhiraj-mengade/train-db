use anyhow::Result;
use async_trait::async_trait;
use libp2p::{Multiaddr, PeerId};
use tracing::{info, warn, error, debug};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(feature = "bluetooth")]
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter};
#[cfg(feature = "bluetooth")]
use btleplug::platform::Manager;
#[cfg(feature = "bluetooth")]
use futures::stream::StreamExt;

// Custom transport trait for multi-transport support
#[async_trait]
pub trait MultiTransport: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    fn get_peer_id(&self) -> PeerId;
    fn get_listen_addresses(&self) -> Vec<Multiaddr>;
    async fn discover_peers(&mut self) -> Result<Vec<PeerId>>;
    async fn connect_to_peer(&mut self, peer_id: PeerId) -> Result<()>;
    async fn send_message(&mut self, peer_id: PeerId, message: Vec<u8>) -> Result<()>;
}

// Bluetooth transport implementation using btleplug
pub struct BluetoothTransport {
    peer_id: PeerId,
    listen_addresses: Vec<Multiaddr>,
    is_running: bool,
    #[cfg(feature = "bluetooth")]
    manager: Option<Manager>,
    #[cfg(feature = "bluetooth")]
    central: Option<btleplug::platform::Adapter>,
    #[cfg(feature = "bluetooth")]
    discovered_devices: Arc<Mutex<HashMap<String, btleplug::platform::Peripheral>>>,
    #[cfg(feature = "bluetooth")]
    connected_peers: Arc<Mutex<HashMap<PeerId, btleplug::platform::Peripheral>>>,
}

impl BluetoothTransport {
    pub fn new() -> Result<Self> {
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        
        Ok(Self {
            peer_id,
            listen_addresses: Vec::new(),
            is_running: false,
            #[cfg(feature = "bluetooth")]
            manager: None,
            #[cfg(feature = "bluetooth")]
            central: None,
            #[cfg(feature = "bluetooth")]
            discovered_devices: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "bluetooth")]
            connected_peers: Arc::new(Mutex::new(HashMap::new())),
        })
    }
}

#[async_trait]
impl MultiTransport for BluetoothTransport {
    async fn start(&mut self) -> Result<()> {
        if self.is_running {
            return Ok(());
        }
        
        #[cfg(feature = "bluetooth")]
        {
            info!("Starting Bluetooth transport with btleplug...");
            
            // Initialize Bluetooth manager
            let manager = Manager::new().await?;
            self.manager = Some(manager);
            
            // Get the first available adapter
            let adapters = self.manager.as_ref().unwrap().adapters().await?;
            if adapters.is_empty() {
                return Err(anyhow::anyhow!("No Bluetooth adapters found"));
            }
            
            let central = adapters.into_iter().next().unwrap();
            self.central = Some(central);
            
            // Start scanning for devices
            let central = self.central.as_ref().unwrap();
            central.start_scan(ScanFilter::default()).await?;
            
            info!("Bluetooth transport started successfully");
            info!("Scanning for TrainDB peers...");
            
            // Start discovery task
            let central = self.central.as_ref().unwrap().clone();
            let discovered_devices = self.discovered_devices.clone();
            let connected_peers = self.connected_peers.clone();
            
            tokio::spawn(async move {
                let mut events = central.events().await.unwrap();
                while let Some(event) = events.next().await {
                    match event {
                        CentralEvent::DeviceDiscovered(device_id) => {
                            info!("🔵 Bluetooth discovered device: {:?}", device_id);
                            if let Ok(peripheral) = central.peripheral(&device_id).await {
                            if let Ok(properties) = peripheral.properties().await {
                                if let Some(props) = properties {
                                    if let Some(name) = &props.local_name {
                                        info!("🔵 Device name: '{}' (ID: {:?})", name, device_id);
                                        
                                        // Look for TrainDB devices OR your specific devices
                                        if name.contains("TrainDB") || 
                                           name.contains("amengade_in-mac") || 
                                           name.contains("Abhiraj Mengade's MacBook Air") ||
                                           name.contains("MacBook") {
                                            info!("🔵 ✅ Found potential TrainDB peer: {} (ID: {:?})", name, device_id);
                                            let mut devices = discovered_devices.lock().await;
                                            devices.insert(device_id.to_string(), peripheral);
                                        } else {
                                            info!("🔵 ⏭️ Skipping device: {} (not a TrainDB peer)", name);
                                        }
                                    } else {
                                        // Also consider devices without names (might be TrainDB nodes)
                                        info!("🔵 Found unnamed device (ID: {:?}) - checking for TrainDB services", device_id);
                                        let mut devices = discovered_devices.lock().await;
                                        devices.insert(device_id.to_string(), peripheral);
                                    }
                                }
                            }
                            }
                        }
                        CentralEvent::DeviceConnected(device_id) => {
                            info!("Connected to device: {:?}", device_id);
                        }
                        CentralEvent::DeviceDisconnected(device_id) => {
                            info!("Disconnected from device: {:?}", device_id);
                            let mut peers = connected_peers.lock().await;
                            peers.retain(|_, peripheral| {
                                peripheral.id() != device_id
                            });
                        }
                        _ => {}
                    }
                }
            });
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled. This is a placeholder implementation.");
        }
        
        self.is_running = true;
        Ok(())
    }
    
    async fn stop(&mut self) -> Result<()> {
        if !self.is_running {
            return Ok(());
        }
        
        #[cfg(feature = "bluetooth")]
        {
            if let Some(central) = &self.central {
                central.stop_scan().await?;
            }
            info!("Bluetooth transport stopped");
        }
        
        self.is_running = false;
        Ok(())
    }
    
    fn get_peer_id(&self) -> PeerId {
        self.peer_id
    }
    
    fn get_listen_addresses(&self) -> Vec<Multiaddr> {
        self.listen_addresses.clone()
    }
    
    async fn discover_peers(&mut self) -> Result<Vec<PeerId>> {
        #[cfg(feature = "bluetooth")]
        {
            let devices = self.discovered_devices.lock().await;
            info!("🔵 Checking {} discovered devices for TrainDB peers", devices.len());
            let mut peers = Vec::new();
            
            for (device_id, peripheral) in devices.iter() {
                if let Ok(properties) = peripheral.properties().await {
                    if let Some(props) = properties {
                        if let Some(name) = &props.local_name {
                            // Accept TrainDB devices or your specific devices
                            if name.contains("TrainDB") || 
                               name.contains("amengade_in-mac") || 
                               name.contains("Abhiraj Mengade's MacBook Air") ||
                               name.contains("MacBook") {
                                info!("🔵 Processing discovered device: {} (ID: {:?})", name, device_id);
                                
                                // Extract peer ID from device name or properties
                                // For now, we'll generate a deterministic peer ID from device ID
                                let device_bytes = hex::decode(device_id.replace("-", "")).unwrap_or_default();
                                let mut key_bytes = [0u8; 32];
                                for (i, &byte) in device_bytes.iter().take(32).enumerate() {
                                    key_bytes[i] = byte;
                                }
                                
                                if let Ok(public_key) = libp2p::identity::ed25519::PublicKey::try_from_bytes(&key_bytes) {
                                    let libp2p_public_key = libp2p::identity::PublicKey::from(public_key);
                                    let peer_id = PeerId::from_public_key(&libp2p_public_key);
                                    peers.push(peer_id);
                                    info!("🔵 Generated peer ID for device {}: {}", name, peer_id);
                                }
                            }
                        } else {
                            // Handle unnamed devices
                            info!("🔵 Processing unnamed device (ID: {:?})", device_id);
                            
                            // Generate peer ID from device ID
                            let device_bytes = hex::decode(device_id.replace("-", "")).unwrap_or_default();
                            let mut key_bytes = [0u8; 32];
                            for (i, &byte) in device_bytes.iter().take(32).enumerate() {
                                key_bytes[i] = byte;
                            }
                            
                            if let Ok(public_key) = libp2p::identity::ed25519::PublicKey::try_from_bytes(&key_bytes) {
                                let libp2p_public_key = libp2p::identity::PublicKey::from(public_key);
                                let peer_id = PeerId::from_public_key(&libp2p_public_key);
                                peers.push(peer_id);
                                info!("🔵 Generated peer ID for unnamed device: {}", peer_id);
                            }
                        }
                    }
                }
            }
            
            Ok(peers)
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled");
            Ok(Vec::new())
        }
    }
    
    async fn connect_to_peer(&mut self, peer_id: PeerId) -> Result<()> {
        #[cfg(feature = "bluetooth")]
        {
            let devices = self.discovered_devices.lock().await;
            for (device_id, peripheral) in devices.iter() {
                if let Ok(properties) = peripheral.properties().await {
                    if let Some(props) = properties {
                        if let Some(name) = &props.local_name {
                            if name.contains("TrainDB") {
                                // Try to connect
                                if let Err(e) = peripheral.connect().await {
                                    error!("Failed to connect to device {}: {}", device_id, e);
                                } else {
                                    info!("Connected to TrainDB peer: {}", name);
                                    let mut peers = self.connected_peers.lock().await;
                                    peers.insert(peer_id, peripheral.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled");
        }
        
        Ok(())
    }
    
    async fn send_message(&mut self, peer_id: PeerId, message: Vec<u8>) -> Result<()> {
        #[cfg(feature = "bluetooth")]
        {
            let peers = self.connected_peers.lock().await;
            if let Some(_peripheral) = peers.get(&peer_id) {
                // Send message via GATT characteristic
                // This is a simplified implementation
                info!("Sending message to peer {}: {} bytes", peer_id, message.len());
                // TODO: Implement actual GATT communication
            } else {
                warn!("Peer {} not connected", peer_id);
            }
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled");
        }
        
        Ok(())
    }
}

// Bluetooth discovery service
pub struct BluetoothDiscovery {
    #[cfg(feature = "bluetooth")]
    manager: Option<Manager>,
    #[cfg(feature = "bluetooth")]
    discovered_peers: Arc<Mutex<HashMap<String, String>>>,
}

impl BluetoothDiscovery {
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "bluetooth")]
            manager: None,
            #[cfg(feature = "bluetooth")]
            discovered_peers: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    pub async fn start_discovery(&mut self) -> Result<()> {
        #[cfg(feature = "bluetooth")]
        {
            info!("Starting Bluetooth discovery...");
            let manager = Manager::new().await?;
            self.manager = Some(manager);
            
            let adapters = self.manager.as_ref().unwrap().adapters().await?;
            if let Some(adapter) = adapters.first() {
                adapter.start_scan(ScanFilter::default()).await?;
                info!("Bluetooth discovery started");
            }
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled");
        }
        
        Ok(())
    }
    
    pub async fn get_discovered_peers(&self) -> HashMap<String, String> {
        #[cfg(feature = "bluetooth")]
        {
            // This would need to be async in a real implementation
            HashMap::new()
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            HashMap::new()
        }
    }
}

// Bluetooth connection manager
pub struct BluetoothConnectionManager {
    #[cfg(feature = "bluetooth")]
    connections: Arc<Mutex<HashMap<String, String>>>,
}

impl BluetoothConnectionManager {
    pub fn new() -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "bluetooth")]
            connections: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    pub async fn connect_to_peer(&mut self, address: String) -> Result<()> {
        #[cfg(feature = "bluetooth")]
        {
            info!("Connecting to Bluetooth peer: {}", address);
            let mut connections = self.connections.lock().await;
            connections.insert(address.clone(), "connected".to_string());
            info!("Connected to peer: {}", address);
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled");
        }
        
        Ok(())
    }
    
    pub async fn disconnect_from_peer(&mut self, address: String) -> Result<()> {
        #[cfg(feature = "bluetooth")]
        {
            info!("Disconnecting from Bluetooth peer: {}", address);
            let mut connections = self.connections.lock().await;
            connections.remove(&address);
            info!("Disconnected from peer: {}", address);
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled");
        }
        
        Ok(())
    }
    
    pub async fn get_connections(&self) -> HashMap<String, String> {
        #[cfg(feature = "bluetooth")]
        {
            // This would need to be async in a real implementation
            HashMap::new()
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            HashMap::new()
        }
    }
}

// Bluetooth message handler
pub struct BluetoothMessageHandler {
    #[cfg(feature = "bluetooth")]
    message_queue: Arc<Mutex<Vec<BluetoothMessage>>>,
}

#[derive(Debug, Clone)]
pub struct BluetoothMessage {
    pub from: String,
    pub to: String,
    pub data: Vec<u8>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl BluetoothMessageHandler {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "bluetooth")]
            message_queue: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub async fn send_message(&self, to: String, data: Vec<u8>) -> Result<()> {
        #[cfg(feature = "bluetooth")]
        {
            info!("Sending Bluetooth message to {}: {} bytes", to, data.len());
            let message = BluetoothMessage {
                from: "local".to_string(),
                to,
                data,
                timestamp: chrono::Utc::now(),
            };
            
            let mut queue = self.message_queue.lock().await;
            queue.push(message);
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled");
        }
        
        Ok(())
    }
    
    pub async fn handle_incoming_messages(&mut self) -> Result<()> {
        #[cfg(feature = "bluetooth")]
        {
            let mut queue = self.message_queue.lock().await;
            while let Some(message) = queue.pop() {
                info!("Received Bluetooth message from {}: {} bytes", 
                      message.from, message.data.len());
                // Process the message here
            }
        }
        
        #[cfg(not(feature = "bluetooth"))]
        {
            warn!("Bluetooth feature not enabled");
        }
        
        Ok(())
    }
}