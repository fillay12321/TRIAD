use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use log::{info, error, debug};

use crate::network::types::{Message, NetworkEvent, PeerInfo, SerializablePeerInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpMessage {
    pub sender_id: String,
    pub message_type: String,
    pub payload: Vec<u8>,
    pub target_shard: Option<usize>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusRequest {
    pub round: usize,
    pub transactions: Vec<Vec<u8>>,
    pub shard_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResponse {
    pub node_id: String,
    pub round: usize,
    pub success: bool,
    pub processed_count: usize,
    pub shard_id: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub node_id: String,
    pub status: String,
    pub port: u16,
    pub active_peers: usize,
    pub shard_id: usize,
}

#[derive(Debug)]
pub struct HttpServer {
    pub node_id: String,
    pub port: u16,
    pub shard_id: usize,
    pub peers: Arc<RwLock<Vec<PeerInfo>>>,
    pub message_queue: Arc<RwLock<Vec<HttpMessage>>>,
}

impl HttpServer {
    pub fn new(node_id: String, port: u16, shard_id: usize) -> Self {
        Self {
            node_id,
            port,
            shard_id,
            peers: Arc::new(RwLock::new(Vec::new())),
            message_queue: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let app = Router::new()
            .route("/health", get(health_handler))
            .route("/consensus", post(consensus_handler))
            .route("/message", post(message_handler))
            .route("/peers", get(peers_handler))
            .route("/peers/:peer_id", post(add_peer_handler))
            .with_state(Arc::new(self.clone()));

        let addr = format!("0.0.0.0:{}", self.port);
        info!("🚀 Starting HTTP server for node {} on {}", self.node_id, addr);
        
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    pub async fn add_peer(&self, peer: PeerInfo) {
        let mut peers = self.peers.write().await;
        if !peers.iter().any(|p| p.id == peer.id) {
            peers.push(peer.clone());
            info!("➕ Added peer: {}:{}", peer.name, peer.address);
        }
    }

    pub async fn broadcast_message(&self, message: HttpMessage) -> Result<(), String> {
        let peers = self.peers.read().await;
        let client = reqwest::Client::new();
        
        for peer in peers.iter() {
            let url = format!("http://{}/message", peer.address);
            if let Err(e) = client.post(&url)
                .json(&message)
                .send()
                .await {
                debug!("Failed to send message to {}: {}", peer.address, e);
            }
        }
        Ok(())
    }
}

impl Clone for HttpServer {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            port: self.port,
            shard_id: self.shard_id,
            peers: Arc::clone(&self.peers),
            message_queue: Arc::clone(&self.message_queue),
        }
    }
}

async fn health_handler(
    State(server): State<Arc<HttpServer>>,
) -> Json<HealthResponse> {
    let peers = server.peers.read().await;
    let response = HealthResponse {
        node_id: server.node_id.clone(),
        status: "healthy".to_string(),
        port: server.port,
        active_peers: peers.len(),
        shard_id: server.shard_id,
    };
    Json(response)
}

async fn consensus_handler(
    State(server): State<Arc<HttpServer>>,
    Json(request): Json<ConsensusRequest>,
) -> Json<ConsensusResponse> {
    info!("📥 Received consensus request for round {} from shard {}", request.round, request.shard_id);
    
    // Обрабатываем транзакции
    let processed_count = request.transactions.len();
    
    let response = ConsensusResponse {
        node_id: server.node_id.clone(),
        round: request.round,
        success: true,
        processed_count,
        shard_id: server.shard_id,
    };
    
    Json(response)
}

async fn message_handler(
    State(server): State<Arc<HttpServer>>,
    Json(message): Json<HttpMessage>,
) -> StatusCode {
    info!("📨 Received message from {}: {}", message.sender_id, message.message_type);
    
    let mut queue = server.message_queue.write().await;
    queue.push(message);
    
    StatusCode::OK
}

async fn peers_handler(
    State(server): State<Arc<HttpServer>>,
) -> Json<Vec<SerializablePeerInfo>> {
    let peers = server.peers.read().await;
    let serializable_peers: Vec<SerializablePeerInfo> = peers.iter().map(|p| p.into()).collect();
    Json(serializable_peers)
}

async fn add_peer_handler(
    State(server): State<Arc<HttpServer>>,
    Path(_peer_id): Path<String>,
    Json(peer_info): Json<SerializablePeerInfo>,
) -> StatusCode {
    // Конвертируем SerializablePeerInfo в PeerInfo
    let peer = PeerInfo::new(
        peer_info.id,
        peer_info.name,
        peer_info.address.parse().unwrap_or_else(|_| "127.0.0.1:8080".parse().unwrap())
    );
    server.add_peer(peer).await;
    StatusCode::OK
}
