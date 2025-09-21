use anyhow::Result;
use libp2p::{
    gossipsub::{self, Gossipsub, GossipsubConfig},
    identify, identity, noise, ping,
    swarm::{NetworkBehaviour, Swarm, SwarmBuilder, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use futures::StreamExt;

use crate::storage::Storage;
use crate::bluetooth::{BluetoothTransport, MultiTransport};

// Main network behavior combining all protocols
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "TrainDBBehaviourEvent")]
pub struct TrainDBBehaviour {
    pub gossipsub: Gossipsub,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
}

#[derive(Debug)]
pub enum TrainDBBehaviourEvent {
    Gossipsub(gossipsub::Event),
    Identify(identify::Event),
    Ping(ping::Event),
}

impl From<gossipsub::Event> for TrainDBBehaviourEvent {
    fn from(event: gossipsub::Event) -> Self {
        TrainDBBehaviourEvent::Gossipsub(event)
    }
}

impl From<identify::Event> for TrainDBBehaviourEvent {
    fn from(event: identify::Event) -> Self {
        TrainDBBehaviourEvent::Identify(event)
    }
}

impl From<ping::Event> for TrainDBBehaviourEvent {
    fn from(event: ping::Event) -> Self {
        TrainDBBehaviourEvent::Ping(event)
    }
}

impl TrainDBBehaviour {
    pub fn new(peer_id: PeerId) -> Self {
        let gossipsub_config = GossipsubConfig::default();
        let gossipsub = Gossipsub::new(
            gossipsub::MessageAuthenticity::Signed(identity::Keypair::generate_ed25519()),
            gossipsub_config,
        )
        .expect("Valid config");
        
        let identify = identify::Behaviour::new(identify::Config::new(
            "/traindb/1.0.0".to_string(),
            identity::Keypair::generate_ed25519().public(),
        ));
        
        let ping = ping::Behaviour::new(ping::Config::new());
        
        Self {
            gossipsub,
            identify,
            ping,
        }
    }
}

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
        
        // Create TCP transport
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::Config::new(&keypair)?)
            .multiplex(yamux::Config::default())
            .boxed();
        
        // Create network behavior
        let behaviour = TrainDBBehaviour::new(peer_id);
        
        // Create swarm
        let swarm = SwarmBuilder::with_tokio_executor(tcp_transport, behaviour, peer_id)
            .build();
        
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
    
    pub fn listen_addresses(&self) -> Vec<Multiaddr> {
        self.swarm.listeners().cloned().collect()
    }
    
    pub async fn add_bootstrap_peer(&mut self, addr: &str) -> Result<()> {
        let multiaddr: Multiaddr = addr.parse()?;
        self.swarm.dial(multiaddr)?;
        Ok(())
    }
    
    pub async fn run(mut self) -> Result<()> {
        info!("Starting TrainDB node event loop");
        
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
            }
        }
        
        Ok(())
    }
    
    async fn handle_swarm_event(&mut self, event: SwarmEvent<TrainDBBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}", address);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connected to peer: {}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                info!("Disconnected from peer: {}", peer_id);
            }
            SwarmEvent::Behaviour(event) => {
                self.handle_behaviour_event(event).await;
            }
            _ => {}
        }
    }
    
    async fn handle_behaviour_event(&mut self, event: TrainDBBehaviourEvent) {
        match event {
            TrainDBBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: _,
                message_id: _,
                message,
            }) => {
                self.handle_gossipsub_message(message).await;
            }
            TrainDBBehaviourEvent::Identify(event) => {
                debug!("Identify event: {:?}", event);
            }
            TrainDBBehaviourEvent::Ping(event) => {
                debug!("Ping event: {:?}", event);
            }
        }
    }
    
    async fn handle_gossipsub_message(&mut self, message: gossipsub::GossipsubMessage) {
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
                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(
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
                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(
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
                if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(
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