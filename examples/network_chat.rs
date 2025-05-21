use std::io::{self, BufRead};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use triad::network::{
    Network, 
    MessageType, 
    MessageHandler, 
    Message, 
    PeerInfo, 
    NetworkError,
    DefaultMessageHandler
};

/// Структура для чат-сообщения
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    /// Имя отправителя
    sender_name: String,
    /// Текст сообщения
    text: String,
}

/// Обработчик сообщений для чата
struct ChatMessageHandler {
    /// Имя текущего пользователя
    username: String,
    /// ID узла
    node_id: Uuid,
    /// Список подключенных узлов
    peers: Arc<Mutex<Vec<PeerInfo>>>,
}

impl ChatMessageHandler {
    /// Создаёт новый обработчик чат-сообщений
    fn new(username: String, node_id: Uuid) -> Self {
        Self {
            username,
            node_id,
            peers: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl MessageHandler for ChatMessageHandler {
    async fn handle_message(&mut self, from: Uuid, message: Message) -> Result<(), NetworkError> {
        match message.message_type {
            MessageType::UserData => {
                // Десериализуем чат-сообщение
                match bincode::deserialize::<ChatMessage>(&message.payload) {
                    Ok(chat_message) => {
                        println!("📨 {} > {}", chat_message.sender_name, chat_message.text);
                    },
                    Err(e) => {
                        eprintln!("❌ Ошибка десериализации сообщения: {}", e);
                    }
                }
            },
            _ => {
                println!("📦 Получено сообщение типа {:?} от {}", message.message_type, from);
            }
        }
        
        Ok(())
    }
    
    async fn handle_peer_connected(&mut self, peer_info: PeerInfo) -> Result<(), NetworkError> {
        println!("✅ Подключен пользователь: {} ({})", peer_info.name, peer_info.id);
        
        // Добавляем узел в список
        let mut peers = self.peers.lock().await;
        peers.push(peer_info);
        
        Ok(())
    }
    
    async fn handle_peer_disconnected(&mut self, peer_id: Uuid) -> Result<(), NetworkError> {
        println!("❌ Отключен пользователь: {}", peer_id);
        
        // Удаляем узел из списка
        let mut peers = self.peers.lock().await;
        peers.retain(|peer| peer.id != peer_id);
        
        Ok(())
    }
}

/// Запускает чат-клиент
async fn run_chat_client(username: String, port: u16) -> Result<(), NetworkError> {
    // Инициализируем логгер
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    // Создаём экземпляр сетевого модуля
    let mut network = Network::new(username.clone(), port).await?;
    
    println!("🔄 Запуск сетевого узла...");
    println!("🆔 ID узла: {}", network.node_id());
    println!("👤 Имя пользователя: {}", username);
    println!("🔌 Порт: {}", port);
    
    // Запускаем сетевой модуль
    network.start().await?;
    
    println!("✅ Сетевой узел запущен. Поиск других узлов...");
    
    // Создаём обработчик сообщений
    let handler = ChatMessageHandler::new(username.clone(), network.node_id());
    let peers = handler.peers.clone();
    
    // Запускаем цикл обработки событий в отдельной задаче
    let mut network_clone = network;
    let event_loop_handle = tokio::spawn(async move {
        if let Err(e) = network_clone.run_event_loop(handler).await {
            eprintln!("❌ Ошибка в цикле обработки событий: {}", e);
        }
    });
    
    // Обрабатываем ввод пользователя
    let network_ref = &network;
    let input_handle = tokio::spawn(async move {
        let stdin = io::stdin();
        let mut lines = stdin.lock().lines();
        
        println!("💬 Введите сообщение (или /exit для выхода):");
        
        while let Some(Ok(line)) = lines.next() {
            if line.trim() == "/exit" {
                break;
            }
            
            if line.trim() == "/peers" {
                let peers_lock = peers.lock().await;
                println!("👥 Подключенные пользователи ({})):", peers_lock.len());
                for (i, peer) in peers_lock.iter().enumerate() {
                    println!("  {}. {} ({})", i + 1, peer.name, peer.id);
                }
                continue;
            }
            
            // Создаём чат-сообщение
            let chat_message = ChatMessage {
                sender_name: username.clone(),
                text: line,
            };
            
            // Сериализуем сообщение
            match bincode::serialize(&chat_message) {
                Ok(payload) => {
                    // Отправляем всем узлам
                    if let Err(e) = network_ref.broadcast(MessageType::UserData, payload).await {
                        eprintln!("❌ Ошибка отправки сообщения: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("❌ Ошибка сериализации сообщения: {}", e);
                }
            }
        }
        
        println!("👋 Выход из чата...");
    });
    
    // Ожидаем завершения задач
    tokio::select! {
        _ = event_loop_handle => {
            println!("🛑 Цикл обработки событий завершён");
        }
        _ = input_handle => {
            println!("🛑 Ввод пользователя завершён");
        }
    }
    
    // Останавливаем сетевой модуль
    network.stop().await?;
    
    println!("👋 Чат-клиент остановлен");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    let (username, port) = if args.len() >= 3 {
        (args[1].clone(), args[2].parse::<u16>().unwrap_or(23000))
    } else {
        println!("Использование: {} <имя_пользователя> [порт]", args[0]);
        println!("Значения по умолчанию: имя_пользователя=User, порт=23000");
        ("User".to_string(), 23000)
    };
    
    run_chat_client(username, port).await?;
    
    Ok(())
} 