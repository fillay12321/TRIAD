use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, interval};
use uuid::Uuid;
use log::{debug, error, info, warn};
use local_ip_address::local_ip;
use serde::{Deserialize, Serialize};

use crate::network::error::NetworkError;
use crate::network::types::{Message, MessageType, NetworkEvent, PeerInfo};
use crate::network::peer::PeerManager;

/// Порт по умолчанию для обнаружения узлов
pub const DEFAULT_DISCOVERY_PORT: u16 = 23456;
/// Интервал между отправками discovery-сообщений
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(15);
/// Размер буфера для приёма сообщений
const BUFFER_SIZE: usize = 1024;

/// Сообщение обнаружения узлов
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiscoveryMessage {
    /// ID узла-отправителя
    node_id: Uuid,
    /// Имя узла-отправителя
    node_name: String,
    /// Порт, на котором узел принимает TCP-соединения
    tcp_port: u16,
}

/// Сервис обнаружения узлов в локальной сети
pub struct DiscoveryService {
    /// ID текущего узла
    node_id: Uuid,
    /// Имя текущего узла
    node_name: String,
    /// Порт для TCP-соединений
    tcp_port: u16,
    /// Канал для отправки событий сети
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Менеджер узлов
    peer_manager: Arc<RwLock<PeerManager>>,
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
            event_sender,
            peer_manager,
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
        
        // Подготавливаем сокет для обнаружения узлов
        let socket = match UdpSocket::bind(SocketAddr::new(local_ip, DEFAULT_DISCOVERY_PORT)).await {
            Ok(socket) => {
                if let Err(e) = socket.set_broadcast(true) {
                    error!("Не удалось включить широковещательный режим: {}", e);
                    return Err(NetworkError::Io(e));
                }
                socket
            },
            Err(e) => {
                error!("Не удалось привязать UDP-сокет: {}", e);
                return Err(NetworkError::Io(e));
            }
        };
        
        let socket = Arc::new(socket);
        let event_sender = self.event_sender.clone();
        let peer_manager = self.peer_manager.clone();
        let node_id = self.node_id;
        let node_name = self.node_name.clone();
        let tcp_port = self.tcp_port;
        
        // Запускаем периодическую отправку discovery-сообщений
        let broadcast_socket = socket.clone();
        tokio::spawn(async move {
            let mut interval = interval(DISCOVERY_INTERVAL);
            
            loop {
                interval.tick().await;
                
                let discovery_msg = DiscoveryMessage {
                    node_id,
                    node_name: node_name.clone(),
                    tcp_port,
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
        
        Ok(())
    }
    
    /// Останавливает сервис обнаружения узлов
    pub async fn stop(&self) -> Result<(), NetworkError> {
        // В реальной реализации здесь нужно остановить все задачи и закрыть сокеты
        // Для простоты примера этот метод пока ничего не делает
        
        info!("Остановлен сервис обнаружения узлов");
        
        Ok(())
    }
} 