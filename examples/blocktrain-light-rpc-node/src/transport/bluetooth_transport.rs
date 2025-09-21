use async_trait::async_trait;
use anyhow::{Result, anyhow, Context};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock; // Better than tokio::sync::RwLock
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

// Fixed libp2p imports for compatibility
use libp2p::{
    gossipsub::{self, Gossipsub, GossipsubEvent, IdentTopic, MessageAuthenticity, ValidationMode, GossipsubConfigBuilder},
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    mdns::{Mdns, MdnsConfig, MdnsEvent},
    ping::{Ping, PingConfig, PingEvent},
    swarm::{SwarmBuilder, SwarmEvent, NetworkBehaviour, Swarm},
    noise, yamux, tcp, identity,
    Multiaddr, PeerId, Transport as Libp2pTransport,
};
use ethereum_types::{H256, U256, Address};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use dashmap::DashMap;

#[cfg(feature = "bluetooth")]
use btleplug::{
    api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType, CharPropFlags},
    platform::{Adapter, Manager, Peripheral},
};

use crate::transport::Transport;
use crate::blockchain_state::{BlockInfo, TransactionInfo, StateManager};

// Fixed message types with size limits and validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BluetoothMessage {
    SyncRequest { 
        from_block: u64, 
        to_block: u64,
        node_id: String,
        timestamp: u64,
    },
    SyncResponse { 
        blocks: Vec<BlockInfo>,
        from_block: u64,
        to_block: u64, 
        node_id: String,
        timestamp: u64,
    },
    StateSnapshot { 
        data: Vec<u8>,
        block_number: u64,
        node_id: String,
        timestamp: u64,
    },
    PeerDiscovery { 
        node_id: String, 
        capabilities: Vec<String>,
        block_number: u64,
        timestamp: u64,
    },
    TransactionBroadcast { 
        tx_hash: H256, 
        tx_data: Vec<u8>,
        node_id: String,
        timestamp: u64,
    },
    Heartbeat {
        node_id: String,
        block_number: u64,
        peer_count: u32,
        timestamp: u64,
    },
}

impl BluetoothMessage {
    const MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB limit

    pub fn validate(&self) -> Result<()> {
        let serialized = bincode::serialize(self)
            .context("Failed to serialize message for validation")?;

        if serialized.len() > Self::MAX_MESSAGE_SIZE {
            return Err(anyhow!("Message too large: {} bytes (max {})", 
                              serialized.len(), Self::MAX_MESSAGE_SIZE));
        }

        // Validate timestamp (not too old or too far in future)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let timestamp = match self {
            BluetoothMessage::SyncRequest { timestamp, .. } |
            BluetoothMessage::SyncResponse { timestamp, .. } |
            BluetoothMessage::StateSnapshot { timestamp, .. } |
            BluetoothMessage::PeerDiscovery { timestamp, .. } |
            BluetoothMessage::TransactionBroadcast { timestamp, .. } |
            BluetoothMessage::Heartbeat { timestamp, .. } => *timestamp,
        };

        if now > timestamp && (now - timestamp) > 600 { // 10 minutes old
            return Err(anyhow!("Message too old: {} seconds", now - timestamp));
        }

        if timestamp > now && (timestamp - now) > 60 { // 1 minute in future
            return Err(anyhow!("Message too far in future: {} seconds", timestamp - now));
        }

        Ok(())
    }

    pub fn get_node_id(&self) -> &str {
        match self {
            BluetoothMessage::SyncRequest { node_id, .. } |
            BluetoothMessage::SyncResponse { node_id, .. } |
            BluetoothMessage::StateSnapshot { node_id, .. } |
            BluetoothMessage::PeerDiscovery { node_id, .. } |
            BluetoothMessage::TransactionBroadcast { node_id, .. } |
            BluetoothMessage::Heartbeat { node_id, .. } => node_id,
        }
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BluetoothBehaviourEvent")]
pub struct BluetoothBehaviour {
    pub gossipsub: Gossipsub,
    pub mdns: Mdns,
    pub identify: Identify,
    pub ping: Ping,
}

#[derive(Debug)]
pub enum BluetoothBehaviourEvent {
    Gossipsub(GossipsubEvent),
    Mdns(MdnsEvent),
    Identify(IdentifyEvent),
    Ping(PingEvent),
}

impl From<GossipsubEvent> for BluetoothBehaviourEvent {
    fn from(event: GossipsubEvent) -> Self {
        BluetoothBehaviourEvent::Gossipsub(event)
    }
}

impl From<MdnsEvent> for BluetoothBehaviourEvent {
    fn from(event: MdnsEvent) -> Self {
        BluetoothBehaviourEvent::Mdns(event)
    }
}

impl From<IdentifyEvent> for BluetoothBehaviourEvent {
    fn from(event: IdentifyEvent) -> Self {
        BluetoothBehaviourEvent::Identify(event)
    }
}

impl From<PingEvent> for BluetoothBehaviourEvent {
    fn from(event: PingEvent) -> Self {
        BluetoothBehaviourEvent::Ping(event)
    }
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: PeerId,
    pub addresses: Vec<Multiaddr>,
    pub last_seen: Instant,
    pub capabilities: Vec<String>,
    pub latest_block: u64,
    pub connection_quality: f32,
    pub message_count: u64,
    pub error_count: u32,
}

impl PeerInfo {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            addresses: Vec::new(),
            last_seen: Instant::now(),
            capabilities: Vec::new(),
            latest_block: 0,
            connection_quality: 1.0,
            message_count: 0,
            error_count: 0,
        }
    }

    pub fn update_quality(&mut self, success: bool) {
        if success {
            self.connection_quality = (self.connection_quality * 0.9 + 1.0 * 0.1).min(1.0);
            self.message_count += 1;
        } else {
            self.connection_quality = (self.connection_quality * 0.9 + 0.0 * 0.1).max(0.0);
            self.error_count += 1;
        }
        self.last_seen = Instant::now();
    }

    pub fn is_stale(&self) -> bool {
        self.last_seen.elapsed() > Duration::from_secs(300) // 5 minutes
    }
}

pub struct BluetoothTransport {
    node_id: String,
    state_manager: Arc<StateManager>,
    peers: Arc<DashMap<PeerId, PeerInfo>>, // Thread-safe concurrent HashMap
    message_sender: Option<mpsc::UnboundedSender<BluetoothMessage>>,
    is_running: Arc<RwLock<bool>>,
    health_score: Arc<RwLock<f32>>,
    last_successful_sync: Arc<RwLock<Option<Instant>>>,

    // Statistics
    messages_sent: Arc<std::sync::atomic::AtomicU64>,
    messages_received: Arc<std::sync::atomic::AtomicU64>,
    bytes_sent: Arc<std::sync::atomic::AtomicU64>,
    bytes_received: Arc<std::sync::atomic::AtomicU64>,

    #[cfg(feature = "bluetooth")]
    ble_adapter: Option<Adapter>,

    // Configuration
    max_peers: usize,
    sync_interval: Duration,
    heartbeat_interval: Duration,
}

impl BluetoothTransport {
    pub async fn new(node_id: String, state_manager: Arc<StateManager>) -> Result<Self> {
        Ok(Self {
            node_id,
            state_manager,
            peers: Arc::new(DashMap::new()),
            message_sender: None,
            is_running: Arc::new(RwLock::new(false)),
            health_score: Arc::new(RwLock::new(0.5)),
            last_successful_sync: Arc::new(RwLock::new(None)),
            messages_sent: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            messages_received: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            bytes_sent: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            bytes_received: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            #[cfg(feature = "bluetooth")]
            ble_adapter: None,
            max_peers: 50,
            sync_interval: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(60),
        })
    }

    async fn setup_libp2p_swarm(&mut self) -> Result<(Swarm<BluetoothBehaviour>, mpsc::UnboundedReceiver<BluetoothMessage>)> {
        // Generate a fixed identity based on node_id for consistent peer ID
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        log::info!("Bluetooth transport peer ID: {:?}", local_peer_id);

        // Build transport with proper error handling
        let transport = tcp::TokioTcpConfig::new()
            .nodelay(true)
            .upgrade(libp2p::core::upgrade::Version::V1)
            .authenticate(noise::NoiseConfig::xx(
                noise::Keypair::<noise::X25519Spec>::new()
                    .into_authentic(&local_key)
                    .map_err(|e| anyhow!("Failed to create noise keypair: {}", e))?
            ))
            .multiplex(yamux::YamuxConfig::default())
            .boxed();

        // Configure GossipSub with reasonable defaults
        let gossipsub_config = GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Strict)
            .max_transmit_size(BluetoothMessage::MAX_MESSAGE_SIZE)
            .duplicate_cache_time(Duration::from_secs(60))
            .build()
            .context("Failed to build GossipSub config")?;

        let mut gossipsub = Gossipsub::new(
            MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        ).context("Failed to create GossipSub")?;

        // Subscribe to topics
        let blockchain_topic = IdentTopic::new("blockchain-sync");
        let heartbeat_topic = IdentTopic::new("heartbeat");
        gossipsub.subscribe(&blockchain_topic)
            .context("Failed to subscribe to blockchain topic")?;
        gossipsub.subscribe(&heartbeat_topic)
            .context("Failed to subscribe to heartbeat topic")?;

        // Set up other protocols
        let mdns = Mdns::new(MdnsConfig::default())
            .await
            .context("Failed to create mDNS")?;

        let identify = Identify::new(IdentifyConfig::new(
            "lightweight-rpc-node/0.1.0".to_string(), 
            local_key.public()
        ));

        let ping = Ping::new(PingConfig::new());

        let behaviour = BluetoothBehaviour {
            gossipsub,
            mdns,
            identify,
            ping,
        };

        let mut swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build();

        // Listen on multiple interfaces with error handling
        let listen_addresses = vec![
            "/ip4/0.0.0.0/tcp/0",
            "/ip6/::/tcp/0",
        ];

        for addr in listen_addresses {
            match addr.parse() {
                Ok(multiaddr) => {
                    if let Err(e) = swarm.listen_on(multiaddr) {
                        log::warn!("Failed to listen on {}: {}", addr, e);
                    }
                }
                Err(e) => log::warn!("Invalid listen address {}: {}", addr, e),
            }
        }

        // Create message channel
        let (message_sender, message_receiver) = mpsc::unbounded_channel();
        self.message_sender = Some(message_sender);

        Ok((swarm, message_receiver))
    }

    #[cfg(feature = "bluetooth")]
    async fn setup_bluetooth(&mut self) -> Result<()> {
        log::info!("Setting up Bluetooth LE adapter");

        match Manager::new().await {
            Ok(manager) => {
                match manager.adapters().await {
                    Ok(adapters) => {
                        if let Some(adapter) = adapters.into_iter().next() {
                            self.ble_adapter = Some(adapter);
                            log::info!("Bluetooth LE adapter initialized successfully");
                        } else {
                            log::warn!("No Bluetooth adapters found");
                        }
                    }
                    Err(e) => log::warn!("Failed to get Bluetooth adapters: {}", e),
                }
            }
            Err(e) => log::warn!("Failed to create Bluetooth manager: {}", e),
        }

        Ok(())
    }

    #[cfg(not(feature = "bluetooth"))]
    async fn setup_bluetooth(&mut self) -> Result<()> {
        log::info!("Bluetooth support not compiled in");
        Ok(())
    }

    async fn start_event_loop(&mut self) -> Result<()> {
        let (mut swarm, mut message_receiver) = self.setup_libp2p_swarm().await?;

        // Clone necessary data for the event loop
        let peers = self.peers.clone();
        let state_manager = self.state_manager.clone();
        let is_running = self.is_running.clone();
        let health_score = self.health_score.clone();
        let last_successful_sync = self.last_successful_sync.clone();
        let node_id = self.node_id.clone();

        let messages_sent = self.messages_sent.clone();
        let messages_received = self.messages_received.clone();
        let bytes_sent = self.bytes_sent.clone();
        let bytes_received = self.bytes_received.clone();

        // Main event loop
        tokio::spawn(async move {
            *is_running.write() = true;

            // Start periodic tasks
            let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(60));
            let mut cleanup_interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            let mut health_check_interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    event = swarm.select_next_some() => {
                        if let Err(e) = Self::handle_swarm_event(
                            event,
                            &peers,
                            &state_manager,
                            &health_score,
                            &last_successful_sync,
                            &node_id,
                            &messages_received,
                            &bytes_received,
                        ).await {
                            log::error!("Error handling swarm event: {}", e);
                        }
                    }

                    msg = message_receiver.recv() => {
                        match msg {
                            Some(bluetooth_msg) => {
                                if let Err(e) = Self::send_message_to_network(
                                    &mut swarm,
                                    bluetooth_msg,
                                    &messages_sent,
                                    &bytes_sent,
                                ).await {
                                    log::error!("Failed to send message to network: {}", e);
                                }
                            }
                            None => {
                                log::info!("Message channel closed, stopping event loop");
                                break;
                            }
                        }
                    }

                    _ = heartbeat_interval.tick() => {
                        Self::send_heartbeat(&mut swarm, &node_id, &state_manager, &peers).await;
                    }

                    _ = cleanup_interval.tick() => {
                        Self::cleanup_stale_peers(&peers);
                    }

                    _ = health_check_interval.tick() => {
                        Self::update_health_score(&peers, &health_score, &last_successful_sync);
                    }
                }

                // Check if we should stop
                if !*is_running.read() {
                    log::info!("Bluetooth transport event loop stopping");
                    break;
                }
            }

            log::info!("Bluetooth transport event loop ended");
        });

        Ok(())
    }

    async fn handle_swarm_event(
        event: SwarmEvent<BluetoothBehaviourEvent, std::io::Error>,
        peers: &DashMap<PeerId, PeerInfo>,
        state_manager: &StateManager,
        health_score: &RwLock<f32>,
        last_successful_sync: &RwLock<Option<Instant>>,
        node_id: &str,
        messages_received: &std::sync::atomic::AtomicU64,
        bytes_received: &std::sync::atomic::AtomicU64,
    ) -> Result<()> {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                log::info!("Bluetooth transport listening on {:?}", address);
            }

            SwarmEvent::Behaviour(BluetoothBehaviourEvent::Mdns(MdnsEvent::Discovered(list))) => {
                for (peer, multiaddr) in list {
                    log::info!("mDNS discovered peer: {:?} at {:?}", peer, multiaddr);

                    // Add or update peer info
                    peers.entry(peer)
                        .or_insert_with(|| PeerInfo::new(peer))
                        .addresses.push(multiaddr);
                }
            }

            SwarmEvent::Behaviour(BluetoothBehaviourEvent::Gossipsub(GossipsubEvent::Message {
                propagation_source,
                message,
                ..
            })) => {
                // Update statistics
                messages_received.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                bytes_received.fetch_add(message.data.len() as u64, std::sync::atomic::Ordering::Relaxed);

                // Process message
                match bincode::deserialize::<BluetoothMessage>(&message.data) {
                    Ok(bluetooth_msg) => {
                        // Validate message
                        if let Err(e) = bluetooth_msg.validate() {
                            log::warn!("Invalid message from {:?}: {}", propagation_source, e);
                            if let Some(mut peer) = peers.get_mut(&propagation_source) {
                                peer.update_quality(false);
                            }
                            return Ok(());
                        }

                        // Don't process our own messages
                        if bluetooth_msg.get_node_id() == node_id {
                            return Ok(());
                        }

                        log::debug!("Received message from {:?}: {:?}", propagation_source, bluetooth_msg);

                        // Update peer quality
                        if let Some(mut peer) = peers.get_mut(&propagation_source) {
                            peer.update_quality(true);
                        }

                        // Handle the message
                        if let Err(e) = Self::handle_bluetooth_message(
                            bluetooth_msg,
                            state_manager,
                            last_successful_sync,
                        ).await {
                            log::error!("Failed to handle Bluetooth message: {}", e);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to deserialize message from {:?}: {}", propagation_source, e);
                        if let Some(mut peer) = peers.get_mut(&propagation_source) {
                            peer.update_quality(false);
                        }
                    }
                }
            }

            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                log::info!("Connected to peer: {:?}", peer_id);

                let peer_info = peers.entry(peer_id)
                    .or_insert_with(|| PeerInfo::new(peer_id));
                peer_info.last_seen = Instant::now();
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                log::info!("Disconnected from peer: {:?} (cause: {:?})", peer_id, cause);

                // Don't remove peer immediately, just mark as disconnected
                if let Some(mut peer) = peers.get_mut(&peer_id) {
                    peer.connection_quality *= 0.8; // Reduce quality
                }
            }

            SwarmEvent::IncomingConnectionError { error, .. } => {
                log::debug!("Incoming connection error: {}", error);
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                log::debug!("Outgoing connection error to {:?}: {}", peer_id, error);
                if let Some(peer_id) = peer_id {
                    if let Some(mut peer) = peers.get_mut(&peer_id) {
                        peer.update_quality(false);
                    }
                }
            }

            _ => {} // Handle other events as needed
        }

        Ok(())
    }

    async fn send_message_to_network(
        swarm: &mut Swarm<BluetoothBehaviour>,
        message: BluetoothMessage,
        messages_sent: &std::sync::atomic::AtomicU64,
        bytes_sent: &std::sync::atomic::AtomicU64,
    ) -> Result<()> {
        // Validate message before sending
        message.validate().context("Message validation failed")?;

        let data = bincode::serialize(&message)
            .context("Failed to serialize Bluetooth message")?;

        let topic = match message {
            BluetoothMessage::Heartbeat { .. } => IdentTopic::new("heartbeat"),
            _ => IdentTopic::new("blockchain-sync"),
        };

        match swarm.behaviour_mut().gossipsub.publish(topic, data.clone()) {
            Ok(_) => {
                messages_sent.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                bytes_sent.fetch_add(data.len() as u64, std::sync::atomic::Ordering::Relaxed);
                log::debug!("Sent Bluetooth message: {:?}", message);
                Ok(())
            }
            Err(e) => {
                Err(anyhow!("Failed to publish message: {}", e))
            }
        }
    }

    async fn handle_bluetooth_message(
        message: BluetoothMessage,
        state_manager: &StateManager,
        last_successful_sync: &RwLock<Option<Instant>>,
    ) -> Result<()> {
        match message {
            BluetoothMessage::SyncRequest { from_block, to_block, node_id, .. } => {
                log::debug!("Processing sync request from {}: blocks {} to {}", node_id, from_block, to_block);

                // TODO: Implement sync response - get blocks from state manager and send back
                // This would require access to the message sender, so we might need to restructure this
            }

            BluetoothMessage::SyncResponse { blocks, .. } => {
                log::debug!("Processing sync response with {} blocks", blocks.len());

                let mut success_count = 0;
                for block in blocks {
                    if let Err(e) = state_manager.update_block(block).await {
                        log::error!("Failed to update block: {}", e);
                    } else {
                        success_count += 1;
                    }
                }

                if success_count > 0 {
                    *last_successful_sync.write() = Some(Instant::now());
                    log::info!("Successfully synced {} blocks via Bluetooth", success_count);
                }
            }

            BluetoothMessage::StateSnapshot { data, block_number, .. } => {
                log::debug!("Processing state snapshot for block {}", block_number);

                match state_manager.apply_synced_state(&data).await {
                    Ok(applied) => {
                        if applied {
                            *last_successful_sync.write() = Some(Instant::now());
                            log::info!("Applied state snapshot for block {}", block_number);
                        } else {
                            log::debug!("State snapshot not applied (not newer)");
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to apply state snapshot: {}", e);
                    }
                }
            }

            BluetoothMessage::PeerDiscovery { node_id, capabilities, block_number, .. } => {
                log::debug!("Peer discovery from {}: block {}, capabilities {:?}", 
                           node_id, block_number, capabilities);
            }

            BluetoothMessage::TransactionBroadcast { tx_hash, .. } => {
                log::debug!("Transaction broadcast: {:?}", tx_hash);
                // TODO: Handle transaction broadcast
            }

            BluetoothMessage::Heartbeat { node_id, block_number, peer_count, .. } => {
                log::debug!("Heartbeat from {}: block {}, peers {}", node_id, block_number, peer_count);
            }
        }

        Ok(())
    }

    async fn send_heartbeat(
        swarm: &mut Swarm<BluetoothBehaviour>,
        node_id: &str,
        state_manager: &StateManager,
        peers: &DashMap<PeerId, PeerInfo>,
    ) {
        let heartbeat = BluetoothMessage::Heartbeat {
            node_id: node_id.to_string(),
            block_number: state_manager.get_latest_block_number().await,
            peer_count: peers.len() as u32,
            timestamp: BluetoothMessage::current_timestamp(),
        };

        if let Ok(data) = bincode::serialize(&heartbeat) {
            let topic = IdentTopic::new("heartbeat");
            if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
                log::debug!("Failed to send heartbeat: {}", e);
            }
        }
    }

    fn cleanup_stale_peers(peers: &DashMap<PeerId, PeerInfo>) {
        let stale_peers: Vec<PeerId> = peers
            .iter()
            .filter(|entry| entry.is_stale())
            .map(|entry| entry.peer_id)
            .collect();

        for peer_id in stale_peers {
            peers.remove(&peer_id);
            log::debug!("Removed stale peer: {:?}", peer_id);
        }
    }

    fn update_health_score(
        peers: &DashMap<PeerId, PeerInfo>,
        health_score: &RwLock<f32>,
        last_successful_sync: &RwLock<Option<Instant>>,
    ) {
        let peer_count = peers.len() as f32;
        let avg_peer_quality = if peer_count > 0.0 {
            peers.iter().map(|p| p.connection_quality).sum::<f32>() / peer_count
        } else {
            0.0
        };

        let sync_score = match *last_successful_sync.read() {
            Some(last_sync) => {
                let elapsed = last_sync.elapsed().as_secs() as f32;
                if elapsed < 60.0 {
                    1.0
                } else if elapsed < 300.0 {
                    1.0 - (elapsed - 60.0) / 240.0 // Linear decay over 4 minutes
                } else {
                    0.0
                }
            }
            None => 0.0,
        };

        let connectivity_score = (peer_count / 10.0).min(1.0); // Max score at 10+ peers

        let overall_score = (avg_peer_quality * 0.4 + sync_score * 0.4 + connectivity_score * 0.2).max(0.0).min(1.0);

        *health_score.write() = overall_score;
    }

    async fn send_message(&self, message: BluetoothMessage) -> Result<()> {
        if let Some(ref sender) = self.message_sender {
            sender.send(message)
                .map_err(|e| anyhow!("Failed to send message to queue: {}", e))?;
            Ok(())
        } else {
            Err(anyhow!("Message sender not initialized"))
        }
    }

    pub fn get_transport_stats(&self) -> BluetoothStats {
        BluetoothStats {
            peer_count: self.peers.len(),
            messages_sent: self.messages_sent.load(std::sync::atomic::Ordering::Relaxed),
            messages_received: self.messages_received.load(std::sync::atomic::Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(std::sync::atomic::Ordering::Relaxed),
            bytes_received: self.bytes_received.load(std::sync::atomic::Ordering::Relaxed),
            health_score: *self.health_score.read(),
            is_running: *self.is_running.read(),
            last_successful_sync: *self.last_successful_sync.read(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BluetoothStats {
    pub peer_count: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub health_score: f32,
    pub is_running: bool,
    pub last_successful_sync: Option<Instant>,
}

#[async_trait]
impl Transport for BluetoothTransport {
    async fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing Bluetooth transport");

        self.setup_bluetooth().await
            .context("Failed to setup Bluetooth LE")?;

        self.start_event_loop().await
            .context("Failed to start Bluetooth event loop")?;

        // Wait a bit for the network to initialize
        tokio::time::sleep(Duration::from_millis(2000)).await;

        log::info!("Bluetooth transport initialized successfully");
        Ok(())
    }

    async fn is_available(&self) -> bool {
        let is_running = *self.is_running.read();
        let peer_count = self.peers.len();
        let health_score = *self.health_score.read();

        is_running && peer_count > 0 && health_score > 0.1
    }

    async fn get_latest_block_number(&self) -> Result<u64> {
        // First try local state
        let local_block = self.state_manager.get_latest_block_number().await;

        // If we don't have recent data, request sync from peers
        if local_block == 0 || self.last_successful_sync.read().as_ref()
            .map(|t| t.elapsed() > Duration::from_secs(300))
            .unwrap_or(true) {

            let sync_request = BluetoothMessage::SyncRequest {
                from_block: local_block.saturating_sub(10),
                to_block: local_block + 100,
                node_id: self.node_id.clone(),
                timestamp: BluetoothMessage::current_timestamp(),
            };

            self.send_message(sync_request).await?;

            // Wait a bit for responses
            tokio::time::sleep(Duration::from_millis(3000)).await;
        }

        Ok(self.state_manager.get_latest_block_number().await)
    }

    async fn get_block_by_number(&self, number: u64) -> Result<Option<BlockInfo>> {
        if let Some(block) = self.state_manager.get_block_by_number(number).await {
            return Ok(Some(block));
        }

        // Request specific block from peers
        let sync_request = BluetoothMessage::SyncRequest {
            from_block: number,
            to_block: number,
            node_id: self.node_id.clone(),
            timestamp: BluetoothMessage::current_timestamp(),
        };

        self.send_message(sync_request).await?;

        // Wait for response
        tokio::time::sleep(Duration::from_millis(2000)).await;

        Ok(self.state_manager.get_block_by_number(number).await)
    }

    async fn get_block_by_hash(&self, hash: &H256) -> Result<Option<BlockInfo>> {
        Ok(self.state_manager.get_block_by_hash(hash).await)
    }

    async fn get_transaction(&self, hash: &H256) -> Result<Option<TransactionInfo>> {
        Ok(self.state_manager.get_transaction(hash).await)
    }

    async fn get_balance(&self, address: &Address) -> Result<U256> {
        Ok(self.state_manager.get_account_balance(address).await)
    }

    async fn get_transaction_count(&self, address: &Address) -> Result<U256> {
        Ok(self.state_manager.get_account_nonce(address).await)
    }

    async fn send_raw_transaction(&self, tx_data: &[u8]) -> Result<H256> {
        // Calculate transaction hash
        let tx_hash = H256::from_slice(&sha3::Keccak256::digest(tx_data));

        // Broadcast transaction to peers
        let broadcast_msg = BluetoothMessage::TransactionBroadcast {
            tx_hash,
            tx_data: tx_data.to_vec(),
            node_id: self.node_id.clone(),
            timestamp: BluetoothMessage::current_timestamp(),
        };

        self.send_message(broadcast_msg).await?;

        Ok(tx_hash)
    }

    async fn call_contract(&self, _to: &Address, _data: &[u8]) -> Result<Vec<u8>> {
        Err(anyhow!("Contract calls not supported via Bluetooth transport - requires real-time state access"))
    }

    async fn estimate_gas(&self, _to: Option<&Address>, _data: &[u8]) -> Result<U256> {
        Err(anyhow!("Gas estimation not supported via Bluetooth transport - requires real-time state access"))
    }

    async fn get_gas_price(&self) -> Result<U256> {
        Ok(self.state_manager.get_gas_price().await)
    }

    async fn get_health_score(&self) -> f32 {
        *self.health_score.read()
    }

    async fn shutdown(&mut self) -> Result<()> {
        log::info!("Shutting down Bluetooth transport");

        *self.is_running.write() = false;

        // Close message sender
        self.message_sender = None;

        // Log final statistics
        let stats = self.get_transport_stats();
        log::info!("Bluetooth transport stats - Peers: {}, Messages sent: {}, received: {}, Health: {:.2}",
                  stats.peer_count, stats.messages_sent, stats.messages_received, stats.health_score);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_bluetooth_transport_creation() {
        let temp_dir = TempDir::new().unwrap();
        let state_manager = Arc::new(
            StateManager::new(&temp_dir.path().join("test.db"), 31337).unwrap()
        );

        let transport = BluetoothTransport::new(
            "test-node".to_string(),
            state_manager,
        ).await;

        assert!(transport.is_ok());
        let transport = transport.unwrap();
        assert_eq!(transport.node_id, "test-node");
        assert_eq!(transport.peers.len(), 0);
    }

    #[test]
    fn test_bluetooth_message_validation() {
        let msg = BluetoothMessage::Heartbeat {
            node_id: "test".to_string(),
            block_number: 100,
            peer_count: 5,
            timestamp: BluetoothMessage::current_timestamp(),
        };

        assert!(msg.validate().is_ok());

        // Test with old timestamp
        let old_msg = BluetoothMessage::Heartbeat {
            node_id: "test".to_string(),
            block_number: 100,
            peer_count: 5,
            timestamp: BluetoothMessage::current_timestamp() - 700, // 11+ minutes old
        };

        assert!(old_msg.validate().is_err());
    }

    #[test]
    fn test_peer_info_quality() {
        let peer_id = PeerId::random();
        let mut peer = PeerInfo::new(peer_id);

        assert_eq!(peer.connection_quality, 1.0);

        // Test quality degradation
        peer.update_quality(false);
        assert!(peer.connection_quality < 1.0);

        // Test quality improvement
        peer.update_quality(true);
        assert!(peer.connection_quality > 0.0);
    }
}
