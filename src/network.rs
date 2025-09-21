use anyhow::Result;
use libp2p::{
    gossipsub::{self, MessageAuthenticity},
    identity, noise,
    swarm::{Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use ::futures::prelude::*;

use crate::storage::Storage;
use crate::bluetooth::{BluetoothTransport, MultiTransport};

// Use GossipSub behaviour directly for now
pub type TrainDBBehaviour = gossipsub::Behaviour;

// Use GossipSub events directly
pub type TrainDBBehaviourEvent = gossipsub::Event;

// TrainDBBehaviour is now a type alias for gossipsub::Behaviour

// Main P2P node
pub struct TrainDBNode {
    swarm: Swarm<TrainDBBehaviour>,
    bluetooth_transport: Option<BluetoothTransport>,
    storage: Box<dyn Storage>,
    command_sender: mpsc::UnboundedSender<NetworkCommand>,
    command_receiver: mpsc::UnboundedReceiver<NetworkCommand>,
}

#[derive(Debug)]
pub enum NetworkCommand {
    SetKey { key: String, value: String },
    GetKey { key: String },
    DeleteKey { key: String },
    ListKeys,
    BroadcastMessage { message: Vec<u8> },
}

impl TrainDBNode {
    pub async fn new<S: Storage + 'static>(storage: S, port: u16) -> Result<Self> {
        // Generate keypair for this node
        let keypair = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(keypair.public());
        
        // Create TCP transport with keep-alive settings
        let tcp_config = tcp::Config::default()
            .nodelay(true);
        
        let tcp_transport = tcp::tokio::Transport::new(tcp_config)
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&keypair)?)
            .multiplex(yamux::Config::default());

        // Create transport
        let transport = tcp_transport.boxed();

        info!("🔗 TCP transport initialized with Bluetooth support available");
        
        // Create network behavior with default configuration
        let gossipsub_config = gossipsub::Config::default();
        
        let mut behaviour = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(identity::Keypair::generate_ed25519()),
            gossipsub_config,
        )
        .expect("Valid config");
        
        // Subscribe to the main topic for Train-DB data
        let topic = gossipsub::IdentTopic::new("traindb-data");
        behaviour.subscribe(&topic).expect("Valid topic");
        
        // Also subscribe to keep-alive topic
        let keep_alive_topic = gossipsub::IdentTopic::new("train-db");
        behaviour.subscribe(&keep_alive_topic).expect("Valid topic");
        
        // Create swarm
        let swarm = Swarm::new(transport, behaviour, peer_id, libp2p::swarm::Config::with_tokio_executor());
        
        // Create command channels
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        
        // Initialize Bluetooth transport
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
        
        let mut node = Self {
            swarm,
            bluetooth_transport,
            storage: Box::new(storage),
            command_sender,
            command_receiver,
        };
        
        // Start listening on TCP
        if port == 0 {
            node.swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        } else {
            node.swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", port).parse()?)?;
        }
        
        // Start Bluetooth transport if available
        if let Some(ref mut bt) = node.bluetooth_transport {
            if let Err(e) = bt.start().await {
                warn!("Failed to start Bluetooth transport: {}", e);
            }
        }
        
        Ok(node)
    }
    
    pub fn peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn local_peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    
    pub fn listen_addresses(&self) -> Vec<Multiaddr> {
        self.swarm.listeners().cloned().collect()
    }
    
    pub async fn add_bootstrap_peer(&mut self, addr: &str) -> Result<()> {
        let multiaddr: Multiaddr = addr.parse()?;
        self.swarm.dial(multiaddr)?;
        Ok(())
    }
    
    pub async fn dial(&mut self, multiaddr: Multiaddr) -> Result<()> {
        self.swarm.dial(multiaddr)?;
        Ok(())
    }
    
    pub async fn run(mut self) -> Result<()> {
        info!("Starting TrainDB node event loop");
        
        // Give the node time to establish initial connections
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        
        // Start a keep-alive task (very frequent to prevent timeouts)
        let mut keep_alive_interval = tokio::time::interval(std::time::Duration::from_secs(5));
        
        // Start Bluetooth peer discovery task
        let mut bluetooth_discovery_interval = tokio::time::interval(std::time::Duration::from_secs(15));
        
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }
                command = self.command_receiver.recv() => {
                    if let Some(cmd) = command {
                        self.handle_command(cmd).await;
                    } else {
                        break;
                    }
                }
                _ = keep_alive_interval.tick() => {
                    // Send a keep-alive message
                    let keep_alive_msg = serde_json::json!({
                        "type": "keep_alive",
                        "peer_id": self.swarm.local_peer_id().to_string(),
                        "timestamp": chrono::Utc::now().timestamp(),
                        "listen_addresses": self.swarm.listeners().cloned().collect::<Vec<_>>()
                    });
                    
                    if let Ok(data) = serde_json::to_vec(&keep_alive_msg) {
                        if let Err(e) = self.swarm.behaviour_mut().publish(
                            gossipsub::IdentTopic::new("traindb-data"),
                            data,
                        ) {
                            debug!("Failed to send keep-alive: {}", e);
                        } else {
                            info!("📡 Sent keep-alive message to maintain connection");
                        }
                    }
                }
                _ = bluetooth_discovery_interval.tick() => {
                    // Discover and connect to Bluetooth peers
                    self.discover_bluetooth_peers().await;
                }
            }
        }
        
        Ok(())
    }
    
    async fn send_database_to_peer(&mut self, peer_id: PeerId) {
        // Get all keys from our database
        match self.storage.list_keys() {
            Ok(keys) => {
                info!("🔵 Sending {} keys to peer {}", keys.len(), peer_id);
                
                for key in &keys {
                    if let Ok(value) = self.storage.get(&key) {
                        let db_msg = serde_json::json!({
                            "type": "database_sync",
                            "key": key,
                            "value": value,
                            "timestamp": chrono::Utc::now().timestamp()
                        });
                        
                        if let Ok(data) = serde_json::to_vec(&db_msg) {
                            if let Some(ref mut bt) = self.bluetooth_transport {
                                if let Err(e) = bt.send_message(peer_id, data).await {
                                    debug!("Failed to send database key {} to peer {}: {}", key, peer_id, e);
                                } else {
                                    info!("🔵 ✅ Sent key '{}' to peer {}", key, peer_id);
                                }
                            }
                        }
                    }
                }
                
                // Send completion message
                let complete_msg = serde_json::json!({
                    "type": "sync_complete",
                    "peer_id": self.swarm.local_peer_id().to_string(),
                    "total_keys": keys.len(),
                    "timestamp": chrono::Utc::now().timestamp()
                });
                
                if let Ok(data) = serde_json::to_vec(&complete_msg) {
                    if let Some(ref mut bt) = self.bluetooth_transport {
                        if let Err(e) = bt.send_message(peer_id, data).await {
                            debug!("Failed to send sync complete message: {}", e);
                        } else {
                            info!("🔵 ✅ Sent sync complete message to peer {}", peer_id);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to list keys for database sync: {}", e);
            }
        }
    }
    
    async fn discover_bluetooth_peers(&mut self) {
        if let Some(ref mut bt) = self.bluetooth_transport {
            info!("🔵 Discovering Bluetooth peers...");
            
            // Start Bluetooth discovery
            if let Err(e) = bt.start().await {
                debug!("Failed to start Bluetooth discovery: {}", e);
                return;
            }
            
            // Discover peers
            match bt.discover_peers().await {
                Ok(peers) => {
                    if !peers.is_empty() {
                        info!("🔵 Found {} Bluetooth peers: {:?}", peers.len(), peers);
                        
                        // Try to connect to discovered peers via Bluetooth
                        for peer_id in peers {
                            info!("🔵 Attempting to connect to Bluetooth peer: {}", peer_id);
                            
                            // Connect via Bluetooth transport (not libp2p)
                            if let Some(ref mut bt) = self.bluetooth_transport {
                                if let Err(e) = bt.connect_to_peer(peer_id).await {
                                    debug!("Failed to connect to Bluetooth peer {}: {}", peer_id, e);
                                } else {
                                    info!("🔵 Successfully connected to Bluetooth peer: {}", peer_id);
                                    
                                    // Mark peer as connected to avoid rediscovery
                                    // TODO: Implement peer tracking in BluetoothTransport
                                    info!("🔵 Peer {} connected successfully", peer_id);
                                    
                                    // Send database sync request to establish the connection
                                    let sync_msg = serde_json::json!({
                                        "type": "sync_request",
                                        "peer_id": self.swarm.local_peer_id().to_string(),
                                        "timestamp": chrono::Utc::now().timestamp()
                                    });
                                    
                                    if let Ok(data) = serde_json::to_vec(&sync_msg) {
                                        if let Err(e) = bt.send_message(peer_id, data).await {
                                            debug!("Failed to send Bluetooth sync request: {}", e);
                                        } else {
                                            info!("🔵 Sent Bluetooth sync request to peer: {}", peer_id);
                                            
                                            // After sending sync request, send our database
                                            // TODO: Fix this - need to restructure to avoid borrowing issues
                                            info!("🔵 Would send database to peer {}", peer_id);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        debug!("🔵 No Bluetooth peers found");
                    }
                }
                Err(e) => {
                    debug!("Failed to discover Bluetooth peers: {}", e);
                }
            }
        }
    }
    
    async fn handle_swarm_event(&mut self, event: SwarmEvent<TrainDBBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("📡 Listening on {}", address);
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                // Check if this is a Bluetooth connection
                let transport_type = if format!("{:?}", endpoint).contains("bluetooth") {
                    "🔵 Bluetooth"
                } else {
                    "📡 TCP/Wi-Fi"
                };
                info!("✅ {} Connected to peer: {}", transport_type, peer_id);
                
                // Give the connection time to fully establish
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                
                // Send a welcome message to establish the connection
                let welcome_msg = serde_json::json!({
                    "type": "welcome",
                    "peer_id": self.swarm.local_peer_id().to_string(),
                    "timestamp": chrono::Utc::now().timestamp()
                });
                
                if let Ok(data) = serde_json::to_vec(&welcome_msg) {
                    if let Err(e) = self.swarm.behaviour_mut().publish(
                        gossipsub::IdentTopic::new("traindb-data"),
                        data,
                    ) {
                        debug!("Failed to send welcome message: {}", e);
                    } else {
                        info!("📤 Sent welcome message to peer: {}", peer_id);
                    }
                }
                
                // Send an immediate keep-alive message
                let keep_alive_msg = serde_json::json!({
                    "type": "keep_alive",
                    "peer_id": self.swarm.local_peer_id().to_string(),
                    "timestamp": chrono::Utc::now().timestamp(),
                    "immediate": true
                });
                
                if let Ok(data) = serde_json::to_vec(&keep_alive_msg) {
                    if let Err(e) = self.swarm.behaviour_mut().publish(
                        gossipsub::IdentTopic::new("traindb-data"),
                        data,
                    ) {
                        debug!("Failed to send immediate keep-alive: {}", e);
                    } else {
                        info!("📡 Sent immediate keep-alive to peer: {}", peer_id);
                    }
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                info!("❌ Disconnected from peer: {} (reason: {:?})", peer_id, cause);
                
                // If it's a timeout, try to reconnect after a delay
                if format!("{:?}", cause).contains("KeepAliveTimeout") {
                    warn!("⚠️ Connection to peer {} timed out (KeepAliveTimeout)", peer_id);
                    // Note: We can't directly reconnect here as we don't have the original address
                    // This would need to be handled by the discovery mechanism
                }
            }
            SwarmEvent::Dialing { peer_id, .. } => {
                debug!("🔄 Dialing peer: {:?}", peer_id);
            }
            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!("⚠️ Failed to connect to peer {:?}: {}", peer_id, error);
            }
            SwarmEvent::Behaviour(event) => {
                self.handle_behaviour_event(event).await;
            }
            _ => {}
        }
    }
    
    async fn handle_behaviour_event(&mut self, event: TrainDBBehaviourEvent) {
        match event {
            gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            } => {
                self.handle_gossipsub_message(message).await;
            }
            _ => {
                debug!("GossipSub event: {:?}", event);
            }
        }
    }
    
    async fn handle_gossipsub_message(&mut self, message: gossipsub::Message) {
        // Handle incoming messages from other peers
        debug!("📨 Received gossiped message from peer: {:?}", message.source);
        
        // Parse and process the message
        if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&message.data) {
            if let Some(msg_type) = data.get("type") {
                match msg_type.as_str() {
                    Some("welcome") => {
                        if let Some(peer_id) = data.get("peer_id") {
                            info!("👋 Received welcome from peer: {}", peer_id);
                        }
                    }
                    Some("keep_alive") => {
                        if let Some(peer_id) = data.get("peer_id") {
                            debug!("💓 Received keep-alive from peer: {}", peer_id);
                        }
                    }
                    Some("bluetooth_hello") => {
                        if let Some(peer_id) = data.get("peer_id") {
                            info!("🔵 Received Bluetooth hello from peer: {}", peer_id);
                        }
                    }
                    Some("sync_request") => {
                        if let Some(peer_id) = data.get("peer_id") {
                            info!("🔵 Received sync request from peer: {}", peer_id);
                            // TODO: Send our database back to the requesting peer
                        }
                    }
                    Some("database_sync") => {
                        if let (Some(key), Some(value)) = (
                            data.get("key").and_then(|v| v.as_str()),
                            data.get("value").and_then(|v| v.as_str()),
                        ) {
                            info!("🔵 Received database sync: {} = {}", key, value);
                            
                            // Store the received data
                            if let Err(e) = self.storage.set(key, value) {
                                error!("Failed to store received data: {}", e);
                            } else {
                                info!("✅ Stored received data: {} = {}", key, value);
                            }
                        }
                    }
                    Some("sync_complete") => {
                        if let Some(total_keys) = data.get("total_keys").and_then(|v| v.as_u64()) {
                            info!("🔵 Received sync complete: {} keys synchronized", total_keys);
                        }
                    }
                    _ => {
                        debug!("📨 Received message type: {}", msg_type);
                    }
                }
            } else if let Some(op) = data.get("operation") {
                match op.as_str() {
                    Some("set") => {
                        if let (Some(key), Some(value)) = (data.get("key"), data.get("value")) {
                            if let (Some(key_str), Some(value_str)) = (key.as_str(), value.as_str()) {
                                info!("🔄 Gossiping: Setting key '{}' = '{}'", key_str, value_str);
                                if let Err(e) = self.storage.set(key_str, value_str) {
                                    error!("Failed to set key: {}", e);
                                } else {
                                    info!("✅ Successfully applied gossiped data: {} = {}", key_str, value_str);
                                }
                            }
                        }
                    }
                    Some("delete") => {
                        if let Some(key) = data.get("key") {
                            if let Some(key_str) = key.as_str() {
                                info!("🔄 Gossiping: Deleting key '{}'", key_str);
                                if let Err(e) = self.storage.delete(key_str) {
                                    error!("Failed to delete key: {}", e);
                                } else {
                                    info!("✅ Successfully applied gossiped deletion: {}", key_str);
                                }
                            }
                        }
                    }
                    _ => {
                        info!("🔄 Gossiping: Unknown operation: {:?}", data);
                    }
                }
            } else {
                debug!("🔄 Gossiping: Non-operation message: {:?}", data);
            }
        } else {
            debug!("🔄 Gossiping: Non-JSON message: {:?}", String::from_utf8_lossy(&message.data));
        }
    }
    
    async fn handle_command(&mut self, command: NetworkCommand) {
        match command {
            NetworkCommand::SetKey { key, value } => {
                if let Err(e) = self.storage.set(&key, &value) {
                    error!("Failed to set key: {}", e);
                    return;
                }
                
                // Broadcast the change to other peers
                let message = serde_json::json!({
                    "operation": "set",
                    "key": key,
                    "value": value,
                    "timestamp": chrono::Utc::now().timestamp()
                });
                
                if let Ok(data) = serde_json::to_vec(&message) {
                    info!("📤 Broadcasting data change: {} = {}", key, value);
                    
                    // Send via GossipSub (TCP/Wi-Fi)
                    if let Err(e) = self.swarm.behaviour_mut().publish(
                        gossipsub::IdentTopic::new("traindb-data"),
                        data.clone(),
                    ) {
                        error!("Failed to broadcast message via GossipSub: {}", e);
                    } else {
                        info!("✅ Successfully broadcasted data change via GossipSub");
                    }
                    
                    // Also send via Bluetooth to connected peers
                    if let Some(ref mut bt) = self.bluetooth_transport {
                        info!("🔵 Broadcasting data change via Bluetooth: {} = {}", key, value);
                        // TODO: Implement actual Bluetooth message sending to connected peers
                        // For now, this is a placeholder - we need to implement peer tracking
                        info!("🔵 Broadcasting data change via Bluetooth to connected peers");
                        // Note: The actual Bluetooth sending would need to be implemented
                        // based on the connected peers from the Bluetooth transport
                    }
                }
            }
            NetworkCommand::GetKey { key } => {
                match self.storage.get(&key) {
                    Ok(Some(value)) => info!("Key '{}' = '{}'", key, value),
                    Ok(None) => info!("Key '{}' not found", key),
                    Err(e) => error!("Failed to get key: {}", e),
                }
            }
            NetworkCommand::DeleteKey { key } => {
                if let Err(e) = self.storage.delete(&key) {
                    error!("Failed to delete key: {}", e);
                    return;
                }
                
                // Broadcast the deletion to other peers
                let message = serde_json::json!({
                    "operation": "delete",
                    "key": key,
                    "timestamp": chrono::Utc::now().timestamp()
                });
                
                if let Ok(data) = serde_json::to_vec(&message) {
                    if let Err(e) = self.swarm.behaviour_mut().publish(
                        gossipsub::IdentTopic::new("traindb-data"),
                        data,
                    ) {
                        error!("Failed to broadcast message: {}", e);
                    }
                }
            }
            NetworkCommand::ListKeys => {
                match self.storage.list_keys() {
                    Ok(keys) => {
                        info!("Keys in database:");
                        for key in keys {
                            info!("  {}", key);
                        }
                    }
                    Err(e) => error!("Failed to list keys: {}", e),
                }
            }
            NetworkCommand::BroadcastMessage { message } => {
                if let Err(e) = self.swarm.behaviour_mut().publish(
                    gossipsub::IdentTopic::new("traindb-data"),
                    message,
                ) {
                    error!("Failed to broadcast message: {}", e);
                }
            }
        }
    }
    
    pub fn get_command_sender(&self) -> mpsc::UnboundedSender<NetworkCommand> {
        self.command_sender.clone()
    }
}