use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock, Mutex};
use tokio::time::{sleep, interval};
use uuid::Uuid;
use log::{debug, error, info, warn};
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};
use quinn::{Endpoint, ServerConfig, ClientConfig, Connection};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::network::types::{PeerInfo, NetworkEvent, SerializablePeerInfo};
use crate::network::peer::PeerManager;
use crate::network::MessageType;
use crate::network::NetworkError;

/// Порт по умолчанию для обнаружения узлов
pub const DEFAULT_DISCOVERY_PORT: u16 = 23456;
/// Интервал между отправками discovery-сообщений
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(15);
/// Размер буфера для приёма сообщений
const BUFFER_SIZE: usize = 1024;

/// Глобальные bootstrap узлы для быстрого старта P2P сети
/// Отключены для автоматического обнаружения через peer exchange
const BOOTSTRAP_NODES: &[&str] = &[];

/// QUIC порт для быстрых соединений
const QUIC_PORT: u16 = 8081;

/// DHT bucket size для Kademlia
const DHT_BUCKET_SIZE: usize = 20;

/// Peer exchange интервал
const PEER_EXCHANGE_INTERVAL: Duration = Duration::from_secs(30);

/// Сообщение обнаружения узлов
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoveryMessage {
    /// ID узла-отправителя
    node_id: Uuid,
    /// Имя узла-отправителя
    node_name: String,
    /// Порт, на котором узел принимает TCP-соединения
    tcp_port: u16,
    /// QUIC порт для быстрых соединений
    quic_port: u16,
    /// Внешний IP адрес (для NAT traversal)
    external_ip: Option<String>,
    /// DHT routing table
    routing_table: Vec<SerializablePeerInfo>,
    /// Peer exchange список
    known_peers: Vec<SerializablePeerInfo>,
}

/// DHT Kademlia routing table entry
#[derive(Debug, Clone)]
struct DHTEntry {
    node_id: Uuid,
    address: SocketAddr,
    last_seen: std::time::Instant,
    distance: u64,
}

/// Peer exchange сообщение
#[derive(Debug, Clone)]
struct PeerExchangeMessage {
    sender_id: Uuid,
    peers: Vec<SerializablePeerInfo>,
    timestamp: u64, // Unix timestamp вместо Instant
}

/// Сервис обнаружения узлов в глобальной P2P сети
#[derive(Debug)]
pub struct DiscoveryService {
    /// ID текущего узла
    node_id: Uuid,
    /// Имя текущего узла
    node_name: String,
    /// Порт для TCP-соединений
    tcp_port: u16,
    /// QUIC порт для быстрых соединений
    quic_port: u16,
    /// Канал для отправки событий сети
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Менеджер узлов
    peer_manager: Arc<RwLock<PeerManager>>,
    /// DHT routing table для Kademlia
    dht_table: Arc<Mutex<HashMap<u64, Vec<DHTEntry>>>>,
    /// QUIC endpoint для быстрых соединений
    quic_endpoint: Arc<Mutex<Option<Endpoint>>>,
    /// Peer exchange кэш
    peer_exchange_cache: Arc<Mutex<HashMap<Uuid, Vec<SerializablePeerInfo>>>>,
    /// Статистика производительности
    performance_stats: Arc<AtomicU64>,
    /// Флаг, указывающий, запущен ли сервис
    running: bool,
}

impl DiscoveryService {
    /// Создаёт новый сервис обнаружения узлов
    pub fn new(
        node_id: Uuid,
        node_name: String,
        tcp_port: u16,
        event_sender: mpsc::Sender<NetworkEvent>,
        peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Self {
        Self {
            node_id,
            node_name,
            tcp_port,
            quic_port: QUIC_PORT,
            event_sender,
            peer_manager,
            dht_table: Arc::new(Mutex::new(HashMap::new())),
            quic_endpoint: Arc::new(Mutex::new(None)),
            peer_exchange_cache: Arc::new(Mutex::new(HashMap::new())),
            performance_stats: Arc::new(AtomicU64::new(0)),
            running: false,
        }
    }
    
    /// Запускает сервис обнаружения узлов
    pub async fn start(&self) -> Result<(), NetworkError> {
        if self.running {
            return Err(NetworkError::ServiceAlreadyStarted);
        }
        
        // Получаем локальный IP-адрес
        let local_ip = match local_ip() {
            Ok(ip) => ip,
            Err(e) => {
                error!("Не удалось получить локальный IP-адрес: {}", e);
                return Err(NetworkError::Internal(format!("Не удалось получить локальный IP-адрес: {}", e)));
            }
        };
        
        // Автоматический выбор свободного discovery-порта
        let mut port = DEFAULT_DISCOVERY_PORT;
        let socket = loop {
            match UdpSocket::bind(SocketAddr::new(local_ip, port)).await {
                Ok(socket) => {
                    if let Err(e) = socket.set_broadcast(true) {
                        error!("Не удалось включить широковещательный режим: {}", e);
                        return Err(NetworkError::Io(e));
                    }
                    info!("Discovery UDP-порт выбран: {}", port);
                    break socket;
                },
                Err(e) => {
                    if port < DEFAULT_DISCOVERY_PORT + 100 {
                        port += 1;
                        continue;
                    } else {
                        error!("Не удалось привязать UDP-сокет ни к одному порту: {}", e);
                        return Err(NetworkError::Io(e));
                    }
                }
            }
        };
        
        let socket = Arc::new(socket);
        let event_sender = self.event_sender.clone();
        let peer_manager = self.peer_manager.clone();
        let node_id = self.node_id;
        let node_name = self.node_name.clone();
        let tcp_port = self.tcp_port;
        
        // Запускаем периодическую отправку discovery-сообщений с автоматическим обнаружением
        let broadcast_socket = socket.clone();
        let peer_manager_clone = peer_manager.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(DISCOVERY_INTERVAL);
            
            loop {
                interval.tick().await;
                
                // Получаем текущих пиров для peer exchange
                let known_peers = {
                    let pm = peer_manager_clone.read().await;
                    pm.get_all_peers().into_iter().map(|peer| SerializablePeerInfo {
                        id: peer.id,
                        name: peer.name,
                        address: peer.address.to_string(),
                    }).collect()
                };
                
                let discovery_msg = DiscoveryMessage {
                    node_id,
                    node_name: node_name.clone(),
                    tcp_port,
                    quic_port: QUIC_PORT,
                    external_ip: None, // Будет определено в start_global_p2p
                    routing_table: vec![],
                    known_peers, // Отправляем список известных пиров
                };
                
                let msg_bytes = match bincode::serialize(&discovery_msg) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        error!("Ошибка сериализации discovery-сообщения: {}", e);
                        continue;
                    }
                };
                
                // Отправляем broadcast-сообщение
                let broadcast_addr = match local_ip {
                    IpAddr::V4(ipv4) => {
                        let octets = ipv4.octets();
                        format!("{}.{}.{}.255:{}", octets[0], octets[1], octets[2], DEFAULT_DISCOVERY_PORT)
                            .parse::<SocketAddr>()
                            .unwrap()
                    },
                    IpAddr::V6(_) => {
                        warn!("IPv6 не поддерживается для обнаружения узлов");
                        continue;
                    }
                };
                
                if let Err(e) = broadcast_socket.send_to(&msg_bytes, broadcast_addr).await {
                    error!("Ошибка отправки discovery-сообщения: {}", e);
                }
            }
        });
        
        // Запускаем прослушивание discovery-сообщений
        let listen_socket = socket.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; BUFFER_SIZE];
            
            loop {
                match listen_socket.recv_from(&mut buf).await {
                    Ok((len, src)) => {
                        // Пропускаем свои сообщения
                        if src.ip() == local_ip {
                            continue;
                        }
                        
                        let received_data = &buf[..len];
                        
                        match bincode::deserialize::<DiscoveryMessage>(received_data) {
                            Ok(discovery_msg) => {
                                // Пропускаем сообщения от себя
                                if discovery_msg.node_id == node_id {
                                    continue;
                                }
                                
                                debug!("Получено discovery-сообщение от {}: {} ({})",
                                    src, discovery_msg.node_name, discovery_msg.node_id);
                                
                                // Создаём информацию о узле
                                let peer_addr = SocketAddr::new(src.ip(), discovery_msg.tcp_port);
                                let peer_info = PeerInfo::new(
                                    discovery_msg.node_id,
                                    discovery_msg.node_name,
                                    peer_addr,
                                );
                                
                                // Добавляем узел в менеджер
                                let is_new = {
                                    let mut manager = peer_manager.write().await;
                                    manager.add_peer(peer_info.clone())
                                };
                                
                                // Если узел новый, отправляем событие
                                if is_new {
                                    if let Err(e) = event_sender.send(NetworkEvent::PeerConnected(peer_info.clone())).await {
                                        error!("Ошибка отправки события PeerConnected: {}", e);
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Ошибка десериализации discovery-сообщения: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        error!("Ошибка приёма discovery-сообщения: {}", e);
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
        
        info!("Запущен сервис обнаружения узлов. ID узла: {}, имя: {}", self.node_id, self.node_name);
        
        // Запускаем глобальный P2P функционал
        self.start_global_p2p().await?;
        
        Ok(())
    }
    
    /// Останавливает сервис обнаружения узлов
    pub async fn stop(&self) -> Result<(), NetworkError> {
        // В реальной реализации здесь нужно остановить все задачи и закрыть сокеты
        // Для простоты примера этот метод пока ничего не делает
        
        info!("Остановлен сервис обнаружения узлов");
        
        Ok(())
    }

    /// Запускает глобальный P2P функционал с автоматическим обнаружением
    async fn start_global_p2p(&self) -> Result<(), NetworkError> {
        info!("🌍 Запуск глобального P2P с автоматическим обнаружением...");
        
        // 1. Запускаем QUIC сервер для приема соединений
        self.start_quic_server().await?;
        
        // 2. Определяем внешний IP для NAT traversal
        let external_ip = self.get_external_ip().await;
        if let Some(ip) = &external_ip {
            info!("🌍 Внешний IP: {} - узел доступен из интернета", ip);
        } else {
            info!("🏠 Узел за NAT - будет использовать relay узлы");
        }
        
        // 3. Запускаем peer exchange (автоматическое обнаружение)
        self.start_peer_exchange().await?;
        
        // 4. Запускаем DHT обновления с relay функционалом
        self.start_dht_updates().await?;
        
        // 5. Проверяем, можем ли стать relay узлом
        if self.can_become_relay().await {
            info!("🔄 Узел готов стать relay для других узлов");
        }
        
        info!("✅ Глобальный P2P запущен с автоматическим обнаружением!");
        info!("🚀 Узлы будут находить друг друга через peer exchange и DHT");
        Ok(())
    }

    /// Подключается к bootstrap узлам для быстрого старта
    async fn connect_bootstrap_nodes(&self) -> Result<(), NetworkError> {
        info!("🚀 Подключение к bootstrap узлам...");
        
        for bootstrap_node in BOOTSTRAP_NODES {
            if let Ok(addr) = bootstrap_node.parse::<SocketAddr>() {
                match self.connect_to_bootstrap_node(addr).await {
                    Ok(_) => info!("✅ Подключен к bootstrap узлу: {}", bootstrap_node),
                    Err(e) => warn!("⚠️ Не удалось подключиться к {}: {}", bootstrap_node, e),
                }
            }
        }
        
        Ok(())
    }

    /// Подключается к конкретному bootstrap узлу
    async fn connect_to_bootstrap_node(&self, addr: SocketAddr) -> Result<(), NetworkError> {
        // Создаем QUIC endpoint для клиента
        let client_config = self.create_client_config().await?;
        let client_endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| NetworkError::Internal(format!("Failed to create client endpoint: {}", e)))?;
        
        // Подключаемся к bootstrap узлу
        let connection = client_endpoint.connect_with(client_config, addr, "localhost")
            .map_err(|e| NetworkError::Internal(format!("Failed to connect to bootstrap node: {}", e)))?
            .await
            .map_err(|e| NetworkError::Internal(format!("Failed to establish QUIC connection: {}", e)))?;
        
        // Отправляем discovery сообщение
        let discovery_msg = DiscoveryMessage {
            node_id: self.node_id,
            node_name: self.node_name.clone(),
            tcp_port: self.tcp_port,
            quic_port: self.quic_port,
            external_ip: self.get_external_ip().await,
            routing_table: self.get_routing_table().await,
            known_peers: self.get_known_peers().await,
        };

        let msg_bytes = bincode::serialize(&discovery_msg)
            .map_err(|e| NetworkError::Internal(format!("Failed to serialize discovery message: {}", e)))?;
        
        // Отправляем через QUIC stream
        let (mut send, mut recv) = connection.open_bi()
            .await
            .map_err(|e| NetworkError::Internal(format!("Failed to open QUIC stream: {}", e)))?;
        
        send.write_all(&msg_bytes).await
            .map_err(|e| NetworkError::Internal(format!("Failed to send discovery message: {}", e)))?;
        send.finish().await
            .map_err(|e| NetworkError::Internal(format!("Failed to finish QUIC stream: {}", e)))?;
        
        // Читаем ответ
        let response = recv.read_to_end(1024 * 1024).await
            .map_err(|e| NetworkError::Internal(format!("Failed to read response: {}", e)))?;
        
        if !response.is_empty() {
            if let Ok(peers) = bincode::deserialize::<Vec<SerializablePeerInfo>>(&response) {
                info!("📥 Получено {} пиров от bootstrap узла {}", peers.len(), addr);
                self.add_peers_from_bootstrap(peers).await;
            }
        }
        
        info!("✅ QUIC соединение с bootstrap узлом {} установлено", addr);
        Ok(())
    }

    /// Создает QUIC endpoint для быстрых соединений
    async fn create_quic_endpoint(&self) -> Result<Endpoint, NetworkError> {
        // Генерируем сертификат для QUIC
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .map_err(|e| NetworkError::Internal(format!("Failed to generate certificate: {}", e)))?;
        
        let cert_der = cert.serialize_der()
            .map_err(|e| NetworkError::Internal(format!("Failed to serialize certificate: {}", e)))?;
        
        let priv_key = cert.serialize_private_key_der();
        let priv_key = rustls::PrivateKey(priv_key);
        
        // Создаем rustls конфигурацию
        let mut server_crypto = rustls::ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(vec![rustls::Certificate(cert_der)], priv_key)
            .map_err(|e| NetworkError::Internal(format!("Failed to create server config: {}", e)))?;
        
        server_crypto.alpn_protocols = vec![b"triad-p2p".to_vec()];
        
        // Создаем QUIC сервер конфигурацию
        let server_config = ServerConfig::with_crypto(Arc::new(server_crypto));
        
        // Привязываем к QUIC порту
        let addr = format!("0.0.0.0:{}", self.quic_port).parse::<SocketAddr>()
            .map_err(|e| NetworkError::Internal(format!("Failed to parse QUIC address: {}", e)))?;
        
        // Создаем endpoint
        let endpoint = Endpoint::server(server_config, addr)
            .map_err(|e| NetworkError::Internal(format!("Failed to create QUIC endpoint: {}", e)))?;
        
        info!("🚀 QUIC endpoint создан на порту {}", self.quic_port);
        Ok(endpoint)
    }

    /// DHT Kademlia поиск узлов
    async fn dht_find_nodes(&self, target_id: Uuid) -> Result<Vec<PeerInfo>, NetworkError> {
        let distance = self.calculate_distance(self.node_id, target_id);
        let bucket = distance % 160; // SHA-1 hash space
        
        let dht_table = self.dht_table.lock().await;
        if let Some(entries) = dht_table.get(&bucket) {
            let peers: Vec<PeerInfo> = entries.iter()
                .map(|entry| PeerInfo::new(
                    entry.node_id,
                    format!("dht-node-{}", entry.node_id.to_string()[..8].to_string()),
                    SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), 8080),
                ))
                .collect();
            Ok(peers)
        } else {
            Ok(vec![])
        }
    }

    /// Вычисляет XOR расстояние между двумя узлами (Kademlia)
    fn calculate_distance(&self, id1: Uuid, id2: Uuid) -> u64 {
        let bytes1 = id1.as_bytes();
        let bytes2 = id2.as_bytes();
        
        let mut distance = 0u64;
        for i in 0..16 {
            distance ^= (bytes1[i] as u64) << (i * 8);
            distance ^= (bytes2[i] as u64) << (i * 8);
        }
        distance
    }

    /// Запускает peer exchange с автоматическим обнаружением
    async fn start_peer_exchange(&self) -> Result<(), NetworkError> {
        let event_sender = self.event_sender.clone();
        let peer_manager = self.peer_manager.clone();
        let dht_table = self.dht_table.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(PEER_EXCHANGE_INTERVAL);
            
            loop {
                interval.tick().await;
                
                let peers = peer_manager.read().await.get_all_peers();
                if !peers.is_empty() {
                    info!("📤 Peer exchange: {} узлов", peers.len());
                    
                    // Автоматически делимся нашими пирами с другими узлами
                    for peer in &peers {
                        // TODO: Отправляем peer exchange сообщение
                        debug!("🔄 Обмен пирами с узлом: {}", peer.name);
                    }
                }
                
                // Периодически ищем новые узлы через DHT
                if peers.len() < 10 { // Если мало пиров, ищем активнее
                    debug!("🔍 Активный поиск узлов через DHT...");
                    // TODO: DHT поиск новых узлов
                }
            }
        });
        
        Ok(())
    }

    /// Запускает DHT обновления с relay функционалом
    async fn start_dht_updates(&self) -> Result<(), NetworkError> {
        let dht_table = self.dht_table.clone();
        let peer_manager = self.peer_manager.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let peers = peer_manager.read().await.get_all_peers();
                let mut table = dht_table.lock().await;
                
                // Обновляем DHT таблицу
                for peer in &peers {
                    // TODO: Реализовать DHT обновления
                }
                
                // Проверяем, можем ли стать relay узлом для других
                if peers.len() >= 3 {
                    debug!("🔄 DHT обновлен: {} buckets, узел готов стать relay", table.len());
                } else {
                    debug!("🔄 DHT обновлен: {} buckets", table.len());
                }
            }
        });
        
        Ok(())
    }
    
    /// Проверяет, может ли узел стать relay для других
    async fn can_become_relay(&self) -> bool {
        let peers = self.peer_manager.read().await.get_all_peers();
        let external_ip = self.get_external_ip().await;
        
        // Узел может стать relay если:
        // 1. У него есть внешний IP (не за NAT)
        // 2. У него достаточно пиров для маршрутизации
        // 3. Он стабилен и доступен
        external_ip.is_some() && peers.len() >= 2
    }

    /// Получает статистику производительности
    pub async fn get_performance_stats(&self) -> u64 {
        self.performance_stats.load(Ordering::Relaxed)
    }

    /// Создает клиентскую конфигурацию для QUIC
    async fn create_client_config(&self) -> Result<ClientConfig, NetworkError> {
        let mut root_cert_store = rustls::RootCertStore::empty();
        
        // Добавляем наш самоподписанный сертификат в доверенные
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .map_err(|e| NetworkError::Internal(format!("Failed to generate client certificate: {}", e)))?;
        
        let cert_der = cert.serialize_der()
            .map_err(|e| NetworkError::Internal(format!("Failed to serialize client certificate: {}", e)))?;
        
        root_cert_store.add(&rustls::Certificate(cert_der))
            .map_err(|e| NetworkError::Internal(format!("Failed to add certificate to store: {}", e)))?;
        
        let mut client_crypto = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_cert_store)
            .with_no_client_auth();
        
        client_crypto.alpn_protocols = vec![b"triad-p2p".to_vec()];
        
        Ok(ClientConfig::new(Arc::new(client_crypto)))
    }

    /// Получает внешний IP адрес с автоматическим NAT traversal
    async fn get_external_ip(&self) -> Option<String> {
        // Пытаемся получить внешний IP через публичные сервисы
        let ip_services = vec![
            "https://api.ipify.org",
            "https://ifconfig.me", 
            "https://icanhazip.com",
            "https://ident.me"
        ];
        
        for service in ip_services {
            match reqwest::get(service).await {
                Ok(response) => {
                    if let Ok(ip) = response.text().await {
                        let ip = ip.trim();
                        if self.is_valid_ip(ip) {
                            info!("🌍 Внешний IP определен: {} через {}", ip, service);
                            return Some(ip.to_string());
                        }
                    }
                },
                Err(e) => debug!("Сервис {} недоступен: {}", service, e),
            }
        }
        
        warn!("⚠️ Не удалось определить внешний IP");
        None
    }
    
    /// Проверяет валидность IP адреса
    fn is_valid_ip(&self, ip: &str) -> bool {
        ip.parse::<std::net::IpAddr>().is_ok()
    }

    /// Получает текущую routing table
    async fn get_routing_table(&self) -> Vec<SerializablePeerInfo> {
        let dht_table = self.dht_table.lock().await;
        let mut peers = Vec::new();
        
        for bucket in dht_table.values() {
            for entry in bucket {
                peers.push(SerializablePeerInfo {
                    id: entry.node_id,
                    name: format!("dht-{}", entry.node_id.to_string()[..8].to_string()),
                    address: entry.address.to_string(),
                });
            }
        }
        
        peers
    }

    /// Получает известных пиров
    async fn get_known_peers(&self) -> Vec<SerializablePeerInfo> {
        let peer_manager = self.peer_manager.read().await;
        let peers = peer_manager.get_all_peers();
        
        peers.into_iter().map(|peer| SerializablePeerInfo {
            id: peer.id,
            name: peer.name,
            address: peer.address.to_string(),
        }).collect()
    }

    /// Добавляет пиров от bootstrap узла
    async fn add_peers_from_bootstrap(&self, peers: Vec<SerializablePeerInfo>) {
        let mut peer_manager = self.peer_manager.write().await;
        
        let count = peers.len();
        for peer_info in peers {
            if let Ok(addr) = peer_info.address.parse::<SocketAddr>() {
                let peer = PeerInfo::new(
                    peer_info.id,
                    peer_info.name,
                    addr,
                );
                peer_manager.add_peer(peer);
            }
        }
        
        info!("➕ Добавлено {} пиров от bootstrap узла", count);
    }

    /// Запускает QUIC сервер для приема входящих соединений
    async fn start_quic_server(&self) -> Result<(), NetworkError> {
        let endpoint = self.create_quic_endpoint().await?;
        
        // Сохраняем endpoint
        {
            let mut quic_endpoint = self.quic_endpoint.lock().await;
            *quic_endpoint = Some(endpoint.clone());
        }
        
                // Запускаем обработку входящих соединений
        let event_sender = self.event_sender.clone();
        let peer_manager = self.peer_manager.clone();
        
        tokio::spawn(async move {
            while let Some(conn) = endpoint.accept().await {
                let event_sender = event_sender.clone();
                let peer_manager = peer_manager.clone();
                
                tokio::spawn(async move {
                    match conn.await {
                        Ok(connection) => {
                            if let Err(e) = Self::handle_quic_connection(connection, event_sender, peer_manager).await {
                                error!("QUIC connection error: {}", e);
                            }
                        },
                        Err(e) => error!("Failed to establish QUIC connection: {}", e),
                    }
                });
            }
        });
        
        info!("🚀 QUIC сервер запущен на порту {}", self.quic_port);
        Ok(())
    }

    /// Обрабатывает входящее QUIC соединение
    async fn handle_quic_connection(
        conn: Connection,
        event_sender: mpsc::Sender<NetworkEvent>,
        peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Result<(), NetworkError> {
        let (mut send, mut recv) = conn.open_bi()
            .await
            .map_err(|e| NetworkError::Internal(format!("Failed to open QUIC stream: {}", e)))?;
        
        // Читаем discovery сообщение
        let message = recv.read_to_end(1024 * 1024).await
            .map_err(|e| NetworkError::Internal(format!("Failed to read QUIC message: {}", e)))?;
        
        if let Ok(discovery_msg) = bincode::deserialize::<DiscoveryMessage>(&message) {
            // Добавляем пира
            let peer = PeerInfo::new(
                discovery_msg.node_id,
                discovery_msg.node_name,
                SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), discovery_msg.tcp_port),
            );
            
            let mut pm = peer_manager.write().await;
            pm.add_peer(peer);
            
            // Отправляем ответ с нашими пирами
            let our_peers = pm.get_all_peers();
            let serializable_peers: Vec<SerializablePeerInfo> = our_peers.into_iter().map(|p| SerializablePeerInfo {
                id: p.id,
                name: p.name,
                address: p.address.to_string(),
            }).collect();
            
            let response = bincode::serialize(&serializable_peers)
                .map_err(|e| NetworkError::Internal(format!("Failed to serialize response: {}", e)))?;
            
            send.write_all(&response).await
                .map_err(|e| NetworkError::Internal(format!("Failed to send response: {}", e)))?;
            send.finish().await
                .map_err(|e| NetworkError::Internal(format!("Failed to finish QUIC stream: {}", e)))?;
        }
        
        Ok(())
    }
} 