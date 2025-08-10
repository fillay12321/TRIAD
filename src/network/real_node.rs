use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use log::{info, error, debug, warn};

use super::http_server::HttpServer;
use super::http_client::HttpClient;
use super::types::PeerInfo;

#[derive(Debug)]
pub struct RealNetworkNode {
    pub id: String,
    pub port: u16,
    pub shard_id: usize,
    pub http_server: Arc<HttpServer>,
    pub http_client: Arc<HttpClient>,
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
            http_server: Arc::new(HttpServer::new(id.clone(), port, shard_id)),
            http_client: Arc::new(HttpClient::new(id, base_url)),
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
        
        // Запускаем HTTP сервер в отдельной задаче
        let server = Arc::clone(&self.http_server);
        let handle = tokio::spawn(async move {
            if let Err(e) = server.start().await {
                error!("HTTP server failed: {}", e);
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
            
            // Также добавляем в HTTP сервер
            self.http_server.add_peer(peer).await;
        }
        Ok(())
    }

    pub async fn connect_to_peer(&self, peer_address: &str) -> Result<(), String> {
        info!("🔗 Connecting to peer: {}", peer_address);
        
        // Проверяем здоровье пира
        match self.http_client.health_check(peer_address).await {
            Ok(health) => {
                                    let peer = PeerInfo::new(
                        uuid::Uuid::new_v4(), // Генерируем новый UUID
                        health.node_id,
                        peer_address.parse().unwrap_or_else(|_| "127.0.0.1:8080".parse().unwrap())
                    );
                
                self.add_peer(peer).await?;
                info!("✅ Successfully connected to peer {}:{}", peer_address, health.port);
                Ok(())
            },
            Err(e) => {
                warn!("⚠️  Failed to connect to peer {}: {}", peer_address, e);
                Err(e)
            },
        }
    }

    pub async fn send_consensus_request(
        &self,
        peer_address: &str,
        round: usize,
        transactions: Vec<Vec<u8>>,
        shard_id: usize,
    ) -> Result<(), String> {
        debug!("📤 Sending consensus request to {}: round {}, shard {}", peer_address, round, shard_id);
        
        match self.http_client.send_consensus_request(peer_address, round, transactions, shard_id).await {
            Ok(response) => {
                info!("✅ Consensus request processed by {}: {} transactions", peer_address, response.processed_count);
                Ok(())
            },
            Err(e) => {
                error!("❌ Consensus request failed to {}: {}", peer_address, e);
                Err(e)
            },
        }
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
        
        let results = self.http_client.broadcast_to_peers(
            &peer_addresses,
            message_type,
            payload,
            target_shard,
        ).await;
        
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let failure_count = results.len() - success_count;
        
        info!("📊 Broadcast results: {} success, {} failure", success_count, failure_count);
        
        if failure_count > 0 {
            warn!("⚠️  Some messages failed to send");
        }
        
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
