use async_trait::async_trait;
use uuid::Uuid;
use crate::network::types::{Message, PeerInfo};
use crate::network::error::NetworkError;

/// Интерфейс обработчика сетевых сообщений
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Обрабатывает полученное сообщение
    async fn handle_message(&mut self, from: Uuid, message: Message) -> Result<(), NetworkError>;
    
    /// Обрабатывает подключение нового узла
    async fn handle_peer_connected(&mut self, peer_info: PeerInfo) -> Result<(), NetworkError>;
    
    /// Обрабатывает отключение узла
    async fn handle_peer_disconnected(&mut self, peer_id: Uuid) -> Result<(), NetworkError>;
}

/// Реализация обработчика по умолчанию для тестов и отладки
pub struct DefaultMessageHandler {
    node_id: Uuid,
}

impl DefaultMessageHandler {
    /// Создаёт новый экземпляр обработчика по умолчанию
    pub fn new(node_id: Uuid) -> Self {
        Self { node_id }
    }
}

#[async_trait]
impl MessageHandler for DefaultMessageHandler {
    async fn handle_message(&mut self, from: Uuid, message: Message) -> Result<(), NetworkError> {
        log::info!(
            "Узел {} получил сообщение от {}: тип={:?}, id={}",
            self.node_id,
            from,
            message.message_type,
            message.id
        );
        Ok(())
    }
    
    async fn handle_peer_connected(&mut self, peer_info: PeerInfo) -> Result<(), NetworkError> {
        log::info!(
            "Узел {} обнаружил новый узел: {} ({})",
            self.node_id,
            peer_info.name,
            peer_info.id
        );
        Ok(())
    }
    
    async fn handle_peer_disconnected(&mut self, peer_id: Uuid) -> Result<(), NetworkError> {
        log::info!(
            "Узел {} потерял соединение с узлом {}",
            self.node_id,
            peer_id
        );
        Ok(())
    }
} 