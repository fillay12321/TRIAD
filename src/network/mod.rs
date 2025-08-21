//! Network module for message exchange between nodes in a local network
//! Provides node discovery, connection establishment, and message exchange

pub mod types;
mod discovery;
mod transport;
mod message;
mod error;
mod peer;
mod handler;
mod quantum_handler;
// HTTP server removed - using WebSocket only
pub mod websocket_server;
pub mod websocket_client;
pub mod real_node;

pub use error::NetworkError;
pub use discovery::LibP2PDiscoveryService;
pub use transport::TransportService;
pub use message::MessageService;
pub use peer::PeerManager;
pub use handler::MessageHandler;
pub use quantum_handler::{QuantumNetworkHandler, QuantumNetworkStats, NetworkInterferenceEvent};

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use log::{debug, error, info};
use std::time::SystemTime;
use crate::transaction::{Transaction, SmartContract};
use crate::quantum::consensus::{ConsensusMessage, ConsensusState};
use crate::sharding::ShardEvent;
use crate::error_analysis::ErrorContext;
use crate::semantic::SemanticAction;
use serde::{Serialize, Deserialize};
use crate::network::types::{PeerInfo, NetworkEvent};

/// Main class of the network module
pub struct Network {
    /// Unique node identifier
    node_id: Uuid,
    /// Node name
    node_name: String,
    /// Node discovery service
    discovery: Arc<LibP2PDiscoveryService>,
    /// Message transport service
    transport: Arc<TransportService>,
    /// Message processing service
    message_service: Arc<MessageService>,
    /// Peer manager for connections to nodes
    peer_manager: Arc<RwLock<PeerManager>>,
    /// Channel for receiving network events
    event_receiver: mpsc::Receiver<NetworkEvent>,
    /// Channel for sending network events
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Message handler
    handler: Arc<RwLock<Box<dyn MessageHandler + Send + Sync>>>,
}

impl Network {
    /// Creates a new instance of the network module
    pub async fn new(node_id: Uuid, node_name: String, port: u16, handler: Box<dyn MessageHandler + Send + Sync>) -> Result<Self, NetworkError> {
        
        // Create channels for events
        let (event_sender, event_receiver) = mpsc::channel(100);
        
        // Create peer manager
        let peer_manager = Arc::new(RwLock::new(PeerManager::new()));
        
        // Create node discovery service
        let discovery = Arc::new(LibP2PDiscoveryService::new(
            node_name.clone(),
            8081, // QUIC port
            event_sender.clone(),
            peer_manager.clone(),
        ));
        
        // Create message transport service
        let transport = Arc::new(TransportService::new(
            node_id,
            port,
            event_sender.clone(),
            peer_manager.clone(),
        ));
        
        // Create message processing service
        let message_service = Arc::new(MessageService::new(
            node_id,
            event_sender.clone(),
            transport.clone(),
        ));
        
        Ok(Self {
            node_id,
            node_name,
            discovery,
            transport,
            message_service,
            peer_manager,
            event_receiver,
            event_sender,
            handler: Arc::new(RwLock::new(handler)),
        })
    }
    
    /// Starts the network module
    pub async fn start(&mut self) -> Result<(), NetworkError> {
        // Start discovery service
        info!("🚀 Starting discovery service...");
        let discovery_clone = Arc::clone(&self.discovery);
        tokio::spawn(discovery_clone.start());
        info!("🚀 Discovery service ready");
        
        // Start transport service
        self.transport.start().await?;
        
        info!("Network module started, node ID: {}, name: {}", self.node_id, self.node_name);
        
        Ok(())
    }
    
    /// Stops the network module
    pub async fn stop(&mut self) -> Result<(), NetworkError> {
        // Stop discovery service
        info!("🛑 Stopping discovery service...");
        self.discovery.stop().await?;
        info!("🛑 Discovery service stopped");
        
        // Stop transport service
        self.transport.stop().await?;
        
        info!("Network module stopped");
        
        Ok(())
    }
    
    /// Sends a message to a specific node
    pub async fn send_to_peer(&self, peer_id: Uuid, message_type: MessageType, payload: Vec<u8>) -> Result<(), NetworkError> {
        self.message_service.send_to_peer(peer_id, message_type, payload).await
    }
    
    /// Broadcasts a message to all known nodes
    pub async fn broadcast(&self, message_type: MessageType, payload: Vec<u8>) -> Result<(), NetworkError> {
        self.message_service.broadcast(message_type, payload).await
    }
    
    /// Returns the list of known nodes
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.peer_manager.read().await.get_all_peers()
    }
    
    /// Processes network events in an infinite loop
    pub async fn run_event_loop(&mut self) -> Result<(), NetworkError> {
        let mut handler = self.handler.write().await;
        info!("Starting network event loop");
        
        while let Some(event) = self.event_receiver.recv().await {
            match event {
                NetworkEvent::MessageReceived { from, message } => {
                    debug!("Received message from {}: type={:?}", from, message.message_type);
                    
                    // Process message using the handler
                    if let Err(e) = handler.handle_message(from, message).await {
                        error!("Error processing message: {}", e);
                    }
                },
                NetworkEvent::PeerConnected(peer_info) => {
                    info!("New node connected: {} ({})", peer_info.name, peer_info.id);
                    
                    if let Err(e) = handler.handle_peer_connected(peer_info).await {
                        error!("Error processing node connection: {}", e);
                    }
                },
                NetworkEvent::PeerDisconnected(peer_id) => {
                    info!("Node disconnected: {}", peer_id);
                    
                    if let Err(e) = handler.handle_peer_disconnected(peer_id).await {
                        error!("Error processing node disconnection: {}", e);
                    }
                },
            }
        }
        
        Ok(())
    }
    
    /// Returns the ID of the current node
    pub fn node_id(&self) -> Uuid {
        self.node_id
    }
    
    /// Returns the name of the current node
    pub fn node_name(&self) -> &str {
        &self.node_name
    }
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Network")
            .field("node_id", &self.node_id)
            .field("node_name", &self.node_name)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Transaction(Transaction),
    SmartContract(SmartContract),
    Consensus(ConsensusMessage),
    ShardEvent(ShardEvent),
    ErrorEvent(ErrorContext),
    SemanticAction(SemanticAction),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct NetworkNode {
    pub id: String,
    pub address: String,
    pub peers: Vec<String>,
    pub transactions: Vec<Transaction>,
    pub contracts: Vec<SmartContract>,
    pub shard_events: Vec<ShardEvent>,
    pub errors: Vec<ErrorContext>,
    pub semantic_actions: Vec<SemanticAction>,
    pub consensus_state: ConsensusState,
}

#[derive(Debug, Clone)]
pub struct NetworkMessage {
    pub id: Uuid,
    pub sender: String,
    pub receiver: String,
    pub message_type: MessageType,
    pub timestamp: SystemTime,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub id: String,
    pub address: String,
    pub initial_peers: Vec<String>,
    pub consensus_threshold: f64,
}

impl NetworkNode {
    pub fn new(config: NodeConfig) -> Self {
        Self {
            id: config.id,
            address: config.address,
            peers: config.initial_peers,
            transactions: Vec::new(),
            contracts: Vec::new(),
            shard_events: Vec::new(),
            errors: Vec::new(),
            semantic_actions: Vec::new(),
            consensus_state: ConsensusState::default(),
        }
    }

    pub fn broadcast(&mut self, message: NetworkMessage) {
        // Implementation of broadcast
    }

    pub fn process_message(&mut self, message: NetworkMessage) {
        match message.message_type {
            MessageType::Transaction(transaction) => {
                self.transactions.push(transaction);
            }
            MessageType::SmartContract(contract) => {
                self.contracts.push(contract);
            }
            MessageType::ShardEvent(event) => {
                self.shard_events.push(event);
            }
            MessageType::ErrorEvent(error) => {
                self.errors.push(error);
            }
            MessageType::SemanticAction(action) => {
                self.semantic_actions.push(action);
            }
            _ => {}
        }
    }
} 