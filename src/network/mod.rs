//! Сетевой модуль для обмена сообщениями между узлами в локальной сети
//! Обеспечивает обнаружение узлов, установление соединений и обмен сообщениями

mod types;
mod discovery;
mod transport;
mod message;
mod error;
mod peer;
mod handler;

pub use types::{Message, MessageType, NetworkEvent, PeerInfo};
pub use error::NetworkError;
pub use discovery::DiscoveryService;
pub use transport::TransportService;
pub use message::MessageService;
pub use peer::PeerManager;
pub use handler::MessageHandler;

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use log::{debug, error, info, warn};

/// Основной класс сетевого модуля
pub struct Network {
    /// Уникальный идентификатор узла
    node_id: Uuid,
    /// Имя узла
    node_name: String,
    /// Сервис обнаружения узлов
    discovery: Arc<DiscoveryService>,
    /// Сервис транспорта сообщений
    transport: Arc<TransportService>,
    /// Сервис обработки сообщений
    message_service: Arc<MessageService>,
    /// Менеджер соединений с узлами
    peer_manager: Arc<RwLock<PeerManager>>,
    /// Канал для приёма событий сети
    event_receiver: mpsc::Receiver<NetworkEvent>,
    /// Канал для отправки событий сети
    event_sender: mpsc::Sender<NetworkEvent>,
}

impl Network {
    /// Создаёт новый экземпляр сетевого модуля
    pub async fn new(node_name: String, port: u16) -> Result<Self, NetworkError> {
        let node_id = Uuid::new_v4();
        
        // Создаём каналы для событий
        let (event_sender, event_receiver) = mpsc::channel(100);
        
        // Создаём менеджер узлов
        let peer_manager = Arc::new(RwLock::new(PeerManager::new()));
        
        // Создаём сервис обнаружения узлов
        let discovery = Arc::new(DiscoveryService::new(
            node_id,
            node_name.clone(),
            port,
            event_sender.clone(),
            peer_manager.clone(),
        ));
        
        // Создаём сервис транспорта
        let transport = Arc::new(TransportService::new(
            node_id,
            port,
            event_sender.clone(),
            peer_manager.clone(),
        ));
        
        // Создаём сервис обработки сообщений
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
        })
    }
    
    /// Запускает сетевой модуль
    pub async fn start(&mut self) -> Result<(), NetworkError> {
        // Запускаем сервис обнаружения
        self.discovery.start().await?;
        
        // Запускаем сервис транспорта
        self.transport.start().await?;
        
        info!("Сетевой модуль запущен, ID узла: {}, имя: {}", self.node_id, self.node_name);
        
        Ok(())
    }
    
    /// Останавливает сетевой модуль
    pub async fn stop(&self) -> Result<(), NetworkError> {
        // Останавливаем сервисы
        self.discovery.stop().await?;
        self.transport.stop().await?;
        
        info!("Сетевой модуль остановлен");
        
        Ok(())
    }
    
    /// Отправляет сообщение конкретному узлу
    pub async fn send_to_peer(&self, peer_id: Uuid, message_type: MessageType, payload: Vec<u8>) -> Result<(), NetworkError> {
        self.message_service.send_to_peer(peer_id, message_type, payload).await
    }
    
    /// Отправляет сообщение всем известным узлам
    pub async fn broadcast(&self, message_type: MessageType, payload: Vec<u8>) -> Result<(), NetworkError> {
        self.message_service.broadcast(message_type, payload).await
    }
    
    /// Возвращает список известных узлов
    pub async fn get_peers(&self) -> Vec<PeerInfo> {
        self.peer_manager.read().await.get_all_peers()
    }
    
    /// Обрабатывает события сети в бесконечном цикле
    pub async fn run_event_loop(&mut self, mut handler: impl MessageHandler + 'static) -> Result<(), NetworkError> {
        info!("Запуск цикла обработки событий сети");
        
        while let Some(event) = self.event_receiver.recv().await {
            match event {
                NetworkEvent::MessageReceived { from, message } => {
                    debug!("Получено сообщение от {}: тип={:?}", from, message.message_type);
                    
                    // Обрабатываем сообщение с помощью хендлера
                    if let Err(e) = handler.handle_message(from, message).await {
                        error!("Ошибка обработки сообщения: {}", e);
                    }
                },
                NetworkEvent::PeerConnected(peer_info) => {
                    info!("Подключен новый узел: {} ({})", peer_info.name, peer_info.id);
                    
                    if let Err(e) = handler.handle_peer_connected(peer_info).await {
                        error!("Ошибка обработки подключения узла: {}", e);
                    }
                },
                NetworkEvent::PeerDisconnected(peer_id) => {
                    info!("Узел отключен: {}", peer_id);
                    
                    if let Err(e) = handler.handle_peer_disconnected(peer_id).await {
                        error!("Ошибка обработки отключения узла: {}", e);
                    }
                },
                _ => {
                    debug!("Получено другое событие сети: {:?}", event);
                }
            }
        }
        
        Ok(())
    }
    
    /// Возвращает ID текущего узла
    pub fn node_id(&self) -> Uuid {
        self.node_id
    }
    
    /// Возвращает имя текущего узла
    pub fn node_name(&self) -> &str {
        &self.node_name
    }
} 