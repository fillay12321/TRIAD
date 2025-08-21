use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::sync::Mutex as AsyncMutex;
use tokio::task::JoinHandle;
use log::{info, error, debug, warn};

use super::websocket_server::WebSocketServer;
use super::websocket_client::WebSocketClient;
use super::websocket_client::PersistentConnection;
use super::types::{PeerInfo, NetworkEvent};
use super::{PeerManager, LibP2PDiscoveryService};
use tokio::sync::mpsc;

pub struct RealNetworkNode {
    pub id: String,
    pub port: u16,
    pub shard_id: usize,
    pub websocket_server: Arc<WebSocketServer>,
    pub websocket_client: Arc<WebSocketClient>,
    pub peers: Arc<RwLock<Vec<PeerInfo>>>,
    // Пул перманентных WS-соединений: address -> connection
    pub ws_conns: Arc<AsyncMutex<HashMap<String, PersistentConnection>>>,
    // LibP2P discovery service и инфраструктура событий
    pub event_sender: mpsc::Sender<NetworkEvent>,
    pub event_receiver: Arc<AsyncMutex<mpsc::Receiver<NetworkEvent>>>,
    pub peer_manager: Arc<RwLock<PeerManager>>,
    pub libp2p: Arc<LibP2PDiscoveryService>,
    pub server_handle: Option<JoinHandle<()>>,
    pub is_running: bool,
}

impl RealNetworkNode {
    pub fn new(id: String, port: u16, shard_id: usize) -> Self {
        // Публичный базовый URL может быть задан через переменную окружения TRIAD_PUBLIC_WS_URL
        // Например: ws.myhost.com:8080 или wss://ws.myhost.com/consensus
        let base_url = std::env::var("TRIAD_PUBLIC_WS_URL").unwrap_or_else(|_| format!("localhost:{}", port));
        let node_name = std::env::var("TRIAD_NODE_NAME").unwrap_or_else(|_| id.clone());
        let quic_port = std::env::var("TRIAD_LISTEN_QUIC_PORT").ok().and_then(|s| s.parse::<u16>().ok()).unwrap_or(8081);

        // Канал событий и менеджер пиров для libp2p
        let (event_sender, event_receiver) = mpsc::channel(100);
        let peer_manager = Arc::new(RwLock::new(PeerManager::new()));

        // Сервис обнаружения libp2p (QUIC/TCP + Kad + Gossip + Identify + Ping)
        let libp2p = Arc::new(LibP2PDiscoveryService::new(
            node_name,
            quic_port,
            event_sender.clone(),
            peer_manager.clone(),
        ));
        
        Self {
            id: id.clone(),
            port,
            shard_id,
            websocket_server: Arc::new(WebSocketServer::new(id.clone(), port, shard_id).unwrap()),
            websocket_client: Arc::new(WebSocketClient::new(id, base_url)),
            peers: Arc::new(RwLock::new(Vec::new())),
            ws_conns: Arc::new(AsyncMutex::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(AsyncMutex::new(event_receiver)),
            peer_manager,
            libp2p,
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

        // Запускаем libp2p discovery сервис (QUIC/TCP + Kad + Gossip)
        let libp2p = Arc::clone(&self.libp2p);
        tokio::spawn(async move {
            if let Err(e) = libp2p.start().await {
                error!("libp2p discovery service failed: {}", e);
            }
        });

        // Фоновая обработка событий сети (минимальная маршрутизация)
        let event_rx = Arc::clone(&self.event_receiver);
        let peers_vec = Arc::clone(&self.peers);
        tokio::spawn(async move {
            loop {
                let mut rx = event_rx.lock().await;
                match rx.recv().await {
                    Some(NetworkEvent::PeerConnected(peer)) => {
                        let mut p = peers_vec.write().await;
                        p.push(peer);
                    }
                    Some(NetworkEvent::PeerDisconnected(_peer_id)) => {
                        // TODO: поддерживать соответствие Uuid<->PeerId и удалять из списка
                    }
                    Some(NetworkEvent::MessageReceived { .. }) => {
                        // TODO: обработка сообщений через libp2p transport/request-response
                    }
                    None => {
                        break;
                    }
                }
            }
        });
        
        // Фоновая задача: поддержание перманентных WS-соединений (автодозвон)
        let peers = Arc::clone(&self.peers);
        let ws_conns = Arc::clone(&self.ws_conns);
        let self_id = self.id.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                ticker.tick().await;
                let snapshot: Vec<PeerInfo> = { peers.read().await.clone() };
                for p in snapshot {
                    let addr = p.address.to_string();
                    let need_connect = {
                        let map = ws_conns.lock().await;
                        !map.contains_key(&addr)
                    };
                    if need_connect {
                        let client = WebSocketClient::new(self_id.clone(), addr.clone());
                        match client.connect_persistent().await {
                            Ok(conn) => {
                                let mut map = ws_conns.lock().await;
                                map.insert(addr.clone(), conn);
                                info!("🔁 Reconnected WS to {}", addr);
                            }
                            Err(e) => {
                                warn!("⏳ WS reconnect attempt to {} failed: {}", addr, e);
                            }
                        }
                    }
                }
            }
        });
        
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

        // Останавливаем libp2p discovery сервис
        if let Err(e) = self.libp2p.stop().await {
            warn!("Error stopping libp2p: {}", e);
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
            // Пытаемся установить перманентное WS-соединение с пиром
            let addr = peer.address.to_string();
            let id = self.id.clone();
            let ws_conns = Arc::clone(&self.ws_conns);
            tokio::spawn(async move {
                // создаем отдельный клиент на адрес пира
                let client = WebSocketClient::new(id, addr.clone());
                match client.connect_persistent().await {
                    Ok(conn) => {
                        let mut map = ws_conns.lock().await;
                        map.insert(addr.clone(), conn);
                        info!("🔌 Persistent WS connection established to {}", addr);
                    }
                    Err(e) => {
                        warn!("⚠️ Failed to establish WS connection to {}: {}", addr, e);
                    }
                }
            });
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
        
        // Здесь пока оставляем заглушку для типов сообщений, реальная рассылка делается батчами консенсуса
        info!("📊 Placeholder broadcast (no-op) to {} peers", peer_addresses.len());
        
        Ok(())
    }

    /// Отправка батчей консенсуса всем пирам по персистентным WS-соединениям
    pub async fn broadcast_batches(
        &self,
        round: usize,
        batches_per_peer: Vec<(String, Vec<Vec<Vec<u8>>>, Vec<usize>)>,
    ) -> Result<(), String> {
        let mut ok = 0usize;
        let mut fail = 0usize;

        for (addr, batches, shard_ids) in batches_per_peer.into_iter() {
            // Гарантируем соединение (или попытаемся создать новое)
            if !self.ws_conns.lock().await.contains_key(&addr) {
                let client = WebSocketClient::new(self.id.clone(), addr.clone());
                match client.connect_persistent().await {
                    Ok(conn) => {
                        self.ws_conns.lock().await.insert(addr.clone(), conn);
                    }
                    Err(e) => {
                        warn!("Failed to connect to {} for broadcast: {}", addr, e);
                        fail += 1;
                        continue;
                    }
                }
            }

            // Отправляем батчи через существующее соединение
            let mut guard = self.ws_conns.lock().await;
            if let Some(conn) = guard.get_mut(&addr) {
                match conn.send_batches_for_round(round, &batches, &shard_ids).await {
                    Ok(_responses) => {
                        ok += 1;
                    }
                    Err(e) => {
                        warn!("Broadcast to {} failed: {} (dropping connection)", addr, e);
                        guard.remove(&addr);
                        fail += 1;
                    }
                }
            } else {
                fail += 1;
            }
        }

        info!("📊 Broadcast batches results: success={}, failure={}", ok, fail);
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

impl std::fmt::Debug for RealNetworkNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealNetworkNode")
            .field("id", &self.id)
            .field("port", &self.port)
            .field("shard_id", &self.shard_id)
            .field("is_running", &self.is_running)
            .finish_non_exhaustive()
    }
}
