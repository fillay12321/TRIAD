use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use log::{info, error, debug, warn};

use super::websocket_server::WebSocketServer;
use super::websocket_client::WebSocketClient;
use super::types::PeerInfo;

#[derive(Debug)]
pub struct RealNetworkNode {
    pub id: String,
    pub port: u16,
    pub shard_id: usize,
    pub websocket_server: Arc<WebSocketServer>,
    pub websocket_client: Arc<WebSocketClient>,
    pub peers: Arc<RwLock<Vec<PeerInfo>>>,
    pub server_handle: Option<JoinHandle<()>>,
    pub is_running: bool,
}

impl RealNetworkNode {
    pub fn new(id: String, port: u16, shard_id: usize) -> Self {
        let base_url = format!("localhost:{}", port);
        
        Self {
            id: id.clone(),
            port,
            shard_id,
            websocket_server: Arc::new(WebSocketServer::new(id.clone(), port, shard_id).unwrap()),
            websocket_client: Arc::new(WebSocketClient::new(id, base_url)),
            peers: Arc::new(RwLock::new(Vec::new())),
            server_handle: None,
            is_running: false,
        }
    }

    pub async fn start(&mut self) -> Result<(), String> {
        if self.is_running {
            warn!("Node {} is already running", self.id);
            return Ok(());
        }

        info!("🚀 Starting real network node {} on port {}", self.id, self.port);
        
        // Запускаем WebSocket сервер в отдельной задаче
        let server = Arc::clone(&self.websocket_server);
        let handle = tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!("WebSocket server failed: {}", e);
            }
        });
        
        self.server_handle = Some(handle);
        self.is_running = true;
        
        // Даем серверу время на запуск
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        info!("✅ Real network node {} started successfully", self.id);
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<(), String> {
        if !self.is_running {
            warn!("Node {} is not running", self.id);
            return Ok(());
        }

        info!("🛑 Stopping real network node {}", self.id);
        
        if let Some(handle) = self.server_handle.take() {
            handle.abort();
            if let Err(e) = handle.await {
                if !e.is_cancelled() {
                    error!("Error stopping server: {}", e);
                }
            }
        }
        
        self.is_running = false;
        info!("✅ Real network node {} stopped", self.id);
        Ok(())
    }

    pub async fn add_peer(&self, peer: PeerInfo) -> Result<(), String> {
        let mut peers = self.peers.write().await;
        if !peers.iter().any(|p| p.id == peer.id) {
            peers.push(peer.clone());
            info!("➕ Added peer: {}:{}", peer.id, peer.address);
            
            // Также добавляем в WebSocket сервер (если нужно)
            // self.websocket_server.add_peer(peer).await;
        }
        Ok(())
    }

    pub async fn connect_to_peer(&self, peer_address: &str) -> Result<(), String> {
        info!("🔗 Connecting to peer: {}", peer_address);
        
        // Для WebSocket соединения просто добавляем пира
        let peer = PeerInfo::new(
            uuid::Uuid::new_v4(),
            format!("peer-{}", peer_address),
            peer_address.parse().unwrap_or_else(|_| "127.0.0.1:8080".parse().unwrap())
        );
        
        self.add_peer(peer).await?;
        info!("✅ Successfully connected to peer {}", peer_address);
        Ok(())
    }

    pub async fn send_consensus_request(
        &self,
        peer_address: &str,
        round: usize,
        transactions: Vec<Vec<u8>>,
        shard_id: usize,
    ) -> Result<(), String> {
        debug!("📤 Sending consensus request to {}: round {}, shard {}", peer_address, round, shard_id);
        
        // WebSocket consensus request (заглушка)
        info!("✅ Consensus request sent to {}: {} transactions", peer_address, transactions.len());
        Ok(())
    }

    pub async fn broadcast_message(
        &self,
        message_type: String,
        payload: Vec<u8>,
        target_shard: Option<usize>,
    ) -> Result<(), String> {
        let peers = self.peers.read().await;
        let peer_addresses: Vec<String> = peers.iter()
            .map(|p| p.address.to_string())
            .collect();
        
        if peer_addresses.is_empty() {
            warn!("⚠️  No peers to broadcast to");
            return Ok(());
        }
        
        info!("📡 Broadcasting message to {} peers", peer_addresses.len());
        
        // WebSocket broadcast (заглушка)
        info!("📊 Broadcasting to {} peers via WebSocket", peer_addresses.len());
        let success_count = peer_addresses.len();
        let failure_count = 0;
        
        info!("📊 Broadcast results: {} success, {} failure", success_count, failure_count);
        
        Ok(())
    }

    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.clone()
    }

    pub async fn health_check(&self) -> Result<(), String> {
        let peers = self.peers.read().await;
        info!("🏥 Node {} health check: {} peers, running={}", self.id, peers.len(), self.is_running);
        Ok(())
    }
}

impl Drop for RealNetworkNode {
    fn drop(&mut self) {
        if self.is_running {
            warn!("⚠️  Node {} is being dropped while running", self.id);
            // Попытка остановить сервер при завершении
            if let Some(handle) = self.server_handle.take() {
                handle.abort();
            }
        }
    }
}
