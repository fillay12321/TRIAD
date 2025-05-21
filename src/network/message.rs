use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;
use log::{debug, error, info};

use crate::network::error::NetworkError;
use crate::network::types::{Message, MessageType, NetworkEvent};
use crate::network::transport::TransportService;

/// Сервис для обработки и маршрутизации сообщений
pub struct MessageService {
    /// ID текущего узла
    node_id: Uuid,
    /// Канал для отправки событий сети
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Транспортный сервис для отправки сообщений
    transport: Arc<TransportService>,
}

impl MessageService {
    /// Создаёт новый сервис сообщений
    pub fn new(
        node_id: Uuid,
        event_sender: mpsc::Sender<NetworkEvent>,
        transport: Arc<TransportService>,
    ) -> Self {
        Self {
            node_id,
            event_sender,
            transport,
        }
    }
    
    /// Отправляет сообщение конкретному узлу
    pub async fn send_to_peer(&self, peer_id: Uuid, message_type: MessageType, payload: Vec<u8>) -> Result<(), NetworkError> {
        // Создаём сообщение
        let message = Message::new(
            self.node_id,
            Some(peer_id),
            message_type,
            payload,
        );
        
        // Отправляем сообщение
        self.transport.send_message(peer_id, message).await
    }
    
    /// Отправляет сообщение всем известным узлам
    pub async fn broadcast(&self, message_type: MessageType, payload: Vec<u8>) -> Result<(), NetworkError> {
        // Создаём сообщение
        let message = Message::new(
            self.node_id,
            None, // Broadcast
            message_type,
            payload,
        );
        
        // Получаем список всех узлов из транспорта (в реальной реализации)
        // Для примера просто логируем факт отправки
        info!("Рассылка сообщения всем узлам: тип={:?}, id={}", message_type, message.id);
        
        // В реальной реализации здесь нужно получить список узлов и отправить каждому
        // Для простоты примера этот метод пока ничего не делает
        
        Ok(())
    }
    
    /// Обрабатывает входящее сообщение
    pub async fn handle_message(&self, from: Uuid, message: Message) -> Result<(), NetworkError> {
        debug!("Обработка сообщения от {}: тип={:?}, id={}", from, message.message_type, message.id);
        
        // В зависимости от типа сообщения выполняем разные действия
        match message.message_type {
            MessageType::Ping => {
                // Отвечаем на ping
                self.send_to_peer(from, MessageType::Pong, vec![]).await?;
            },
            MessageType::Pong => {
                // Обновляем время последнего пинга (в реальной реализации)
            },
            MessageType::Discovery => {
                // Обрабатываем discovery-сообщение (в реальной реализации)
            },
            MessageType::DiscoveryResponse => {
                // Обрабатываем ответ на discovery-сообщение (в реальной реализации)
            },
            MessageType::UserData => {
                // Передаём пользовательские данные в обработчик (через событие)
                if let Err(e) = self.event_sender.send(NetworkEvent::MessageReceived {
                    from,
                    message,
                }).await {
                    error!("Ошибка отправки события MessageReceived: {}", e);
                }
            },
            _ => {
                // Все остальные типы сообщений тоже передаём в обработчик
                if let Err(e) = self.event_sender.send(NetworkEvent::MessageReceived {
                    from,
                    message,
                }).await {
                    error!("Ошибка отправки события MessageReceived: {}", e);
                }
            }
        }
        
        Ok(())
    }
} 