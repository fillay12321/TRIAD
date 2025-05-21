use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;
use std::time::{Duration, Instant};

/// Типы сообщений, поддерживаемые сетевым модулем
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Ping-сообщение для проверки соединения
    Ping,
    /// Pong-ответ на ping
    Pong,
    /// Обнаружение узлов
    Discovery,
    /// Ответ на обнаружение
    DiscoveryResponse,
    /// Пользовательские данные
    UserData,
    /// Подтверждение получения сообщения
    Ack,
    /// Сообщения, связанные с консенсусом
    Consensus,
    /// Транзакция
    Transaction,
    /// Блок
    Block,
}

/// Сетевое сообщение
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Уникальный идентификатор сообщения
    pub id: Uuid,
    /// Идентификатор отправителя
    pub sender_id: Uuid,
    /// Идентификатор получателя (если None, то broadcast)
    pub recipient_id: Option<Uuid>,
    /// Тип сообщения
    pub message_type: MessageType,
    /// Timestamp сообщения
    pub timestamp: u64,
    /// Полезная нагрузка сообщения
    pub payload: Vec<u8>,
}

impl Message {
    /// Создаёт новое сообщение
    pub fn new(
        sender_id: Uuid,
        recipient_id: Option<Uuid>,
        message_type: MessageType,
        payload: Vec<u8>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_secs();

        Self {
            id: Uuid::new_v4(),
            sender_id,
            recipient_id,
            message_type,
            timestamp: now,
            payload,
        }
    }
}

/// Информация об узле сети
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    /// Уникальный идентификатор узла
    pub id: Uuid,
    /// Имя узла
    pub name: String,
    /// Адрес и порт узла
    pub address: SocketAddr,
    /// Время последнего взаимодействия
    #[serde(skip)]
    pub last_seen: Instant,
}

impl PeerInfo {
    /// Создаёт новую информацию об узле
    pub fn new(id: Uuid, name: String, address: SocketAddr) -> Self {
        Self {
            id,
            name,
            address,
            last_seen: Instant::now(),
        }
    }
    
    /// Обновляет время последнего взаимодействия
    pub fn update_last_seen(&mut self) {
        self.last_seen = Instant::now();
    }
    
    /// Проверяет, активен ли узел
    pub fn is_active(&self, timeout: Duration) -> bool {
        self.last_seen.elapsed() < timeout
    }
}

/// События сетевого модуля
#[derive(Debug)]
pub enum NetworkEvent {
    /// Получено сообщение
    MessageReceived {
        /// Идентификатор отправителя
        from: Uuid,
        /// Само сообщение
        message: Message,
    },
    /// Подключён новый узел
    PeerConnected(PeerInfo),
    /// Узел отключён
    PeerDisconnected(Uuid),
    /// Ошибка сети
    Error(String),
} 