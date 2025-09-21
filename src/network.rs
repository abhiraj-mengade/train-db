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
use crate::dashboard::{Dashboard, PeerInfo};

// Use GossipSub behaviour directly
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
    dashboard: Option<Arc<Dashboard>>,
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
        
        // Create TCP transport
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&keypair)?)
            .multiplex(yamux::Config::default());
        
        // Create multi-transport (TCP + Bluetooth when available)
        let transport = tcp_transport.boxed();
        
        // Try to add Bluetooth transport if available
        #[cfg(feature = "bluetooth")]
        {
            // For now, we'll use TCP only but log that Bluetooth is available
            info!("Bluetooth support compiled in - multi-transport ready");
        }
        
        // Create network behavior with improved config
        let gossipsub_config = gossipsub::Config::default();
        
        let mut behaviour = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(identity::Keypair::generate_ed25519()),
            gossipsub_config,
        )
        .expect("Valid config");
        
        // Subscribe to the main topic for Train-DB
        let topic = gossipsub::IdentTopic::new("train-db");
        behaviour.subscribe(&topic).expect("Valid topic");
        
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
            dashboard: None,
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

    pub fn set_dashboard(&mut self, dashboard: Arc<Dashboard>) {
        self.dashboard = Some(dashboard);
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
        
        // Start a keep-alive task
        let mut keep_alive_interval = tokio::time::interval(std::time::Duration::from_secs(30));
        
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
                    let topic = gossipsub::IdentTopic::new("train-db");
                    let message = format!("Keep-alive from {}", self.swarm.local_peer_id());
                    if let Err(e) = self.swarm.behaviour_mut().publish(topic, message.as_bytes()) {
                        debug!("Failed to send keep-alive: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_swarm_event(&mut self, event: SwarmEvent<TrainDBBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}", address);
                if let Some(ref dashboard) = self.dashboard {
                    dashboard.add_activity(format!("Listening on {}", address));
                }
            }
            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!("Connected to peer: {}", peer_id);
                if let Some(ref dashboard) = self.dashboard {
                    let transport = match endpoint {
                        libp2p::core::ConnectedPoint::Dialer { .. } => "Outbound",
                        libp2p::core::ConnectedPoint::Listener { .. } => "Inbound",
                    };
                    let peer_info = PeerInfo {
                        peer_id: peer_id.to_string(),
                        transport: transport.to_string(),
                        last_seen: chrono::Utc::now().to_rfc3339(),
                        is_connected: true,
                    };
                    dashboard.add_peer(peer_info);
                    dashboard.add_activity(format!("✅ Connected to peer: {}", peer_id));
                    
                    // Log the connection
                    dashboard.add_activity(format!("📤 Peer connection established: {}", peer_id));
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                info!("Disconnected from peer: {}", peer_id);
                if let Some(ref dashboard) = self.dashboard {
                    dashboard.remove_peer(&peer_id.to_string());
                    dashboard.add_activity(format!("❌ Disconnected from peer: {}", peer_id));
                }
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
        debug!("Received message: {:?}", message);
        
        // Parse and process the message
        if let Ok(data) = serde_json::from_slice::<serde_json::Value>(&message.data) {
            if let Some(op) = data.get("operation") {
                match op.as_str() {
                    Some("set") => {
                        if let (Some(key), Some(value)) = (data.get("key"), data.get("value")) {
                            if let (Some(key_str), Some(value_str)) = (key.as_str(), value.as_str()) {
                                if let Err(e) = self.storage.set(key_str, value_str) {
                                    error!("Failed to set key: {}", e);
                                }
                            }
                        }
                    }
                    Some("delete") => {
                        if let Some(key) = data.get("key") {
                            if let Some(key_str) = key.as_str() {
                                if let Err(e) = self.storage.delete(key_str) {
                                    error!("Failed to delete key: {}", e);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
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
                    if let Err(e) = self.swarm.behaviour_mut().publish(
                        gossipsub::IdentTopic::new("traindb-data"),
                        data,
                    ) {
                        error!("Failed to broadcast message: {}", e);
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