use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;
use log::{info, error};

// Импортируем наши модули
use triad::network::{
    Network, NetworkError, MessageType, 
    types::{PeerInfo, Message}, MessageHandler
};

#[derive(Debug)]
pub struct TestnetNode {
    pub network: Arc<Network>,
    pub discovery_port: u16,
    pub tcp_port: u16,
}

impl TestnetNode {
    pub async fn new(discovery_port: u16, tcp_port: u16) -> Result<Self, NetworkError> {
        let node_name = format!("testnet-node-{}", Uuid::new_v4().to_string()[..8].to_string());
        
        // Создаем основную сеть с discovery и transport сервисами
        let network = Network::new(node_name, tcp_port).await?;
        
        Ok(Self {
            network: Arc::new(network),
            discovery_port,
            tcp_port,
        })
    }

    /// Инициализируем тестнет с автоматическим обнаружением узлов
    pub async fn initialize_testnet(&mut self) -> Result<(), NetworkError> {
        println!("🚀 Инициализация TRIAD тестнета...");
        println!("   📡 Discovery порт: {}", self.discovery_port);
        println!("   🌐 TCP порт: {}", self.tcp_port);
        println!("   🏷️  Имя узла: {}", self.network.node_name());
        
        // 1. Запускаем основную сеть (включает discovery и transport сервисы)
        println!("   🔧 Запуск сетевых сервисов...");
        // Получаем mutable reference для запуска
        let network_ref = Arc::get_mut(&mut self.network)
            .ok_or_else(|| NetworkError::Internal("Cannot get mutable reference to network".to_string()))?;
        network_ref.start().await?;
        println!("   ✅ Сетевые сервисы запущены");
        
        // 2. Запускаем event loop для обработки сетевых событий
        println!("   🔄 Запуск event loop...");
        let network_clone = self.network.clone();
        
        // Запускаем event loop в отдельной задаче
        tokio::spawn(async move {
            let mut network_clone = network_clone;
            let handler = TestnetMessageHandler::new(network_clone.node_id());
            // Получаем mutable reference для event loop
            if let Some(network_mut) = Arc::get_mut(&mut network_clone) {
                if let Err(e) = network_mut.run_event_loop(handler).await {
                    error!("Event loop error: {}", e);
                }
            }
        });
        
        println!("   ✅ Event loop запущен");
        
        // 3. Даем время на обнаружение других узлов
        println!("   🔍 Поиск других узлов в тестнете...");
        sleep(Duration::from_secs(5)).await;
        
        // 4. Показываем статус тестнета
        let peers = self.network.get_peers().await;
        println!("   📊 Статус тестнета:");
        println!("      🌐 Локальный узел: {} (порт {})", 
                self.network.node_name(), self.tcp_port);
        println!("      🔗 Обнаружено пиров: {}", peers.len());
        for peer in &peers {
            println!("         - {}: {} (последний раз: {:.1}s назад)", 
                    peer.name, peer.address, 
                    peer.last_seen.elapsed().as_secs_f64());
        }
        
        println!("🎉 Тестнет инициализирован! {} узлов готовы к работе", 1 + peers.len());
        Ok(())
    }

    /// Отправляем тестовое сообщение всем пирам
    pub async fn send_test_message(&self) -> Result<(), NetworkError> {
        let peers = self.network.get_peers().await;
        if peers.is_empty() {
            println!("   💤 Нет пиров для отправки сообщения");
            return Ok(());
        }
        
        println!("   📤 Отправка тестового сообщения {} пирам...", peers.len());
        
        let test_message = format!("Тест от узла {}! Время: {:?}", 
                                 self.network.node_name(), 
                                 std::time::SystemTime::now());
        
        for peer in &peers {
            match self.network.send_to_peer(peer.id, MessageType::Error(test_message.clone()), 
                                          test_message.as_bytes().to_vec()).await {
                Ok(_) => println!("     ✅ Сообщение отправлено пиру {}: {}", peer.name, peer.address),
                Err(e) => println!("     ❌ Ошибка отправки пиру {}: {}", peer.name, e),
            }
        }
        
        Ok(())
    }

    /// Останавливаем тестнет
    pub async fn shutdown(&mut self) -> Result<(), NetworkError> {
        println!("\n🛑 Остановка тестнета...");
        
        // Останавливаем основную сеть
        if let Some(network_mut) = Arc::get_mut(&mut self.network) {
            network_mut.stop().await?;
        }
        
        println!("✅ Тестнет остановлен");
        Ok(())
    }
}

/// Обработчик сообщений для тестнета
pub struct TestnetMessageHandler {
    node_id: Uuid,
}

impl TestnetMessageHandler {
    pub fn new(node_id: Uuid) -> Self {
        Self { node_id }
    }
}

#[async_trait::async_trait]
impl MessageHandler for TestnetMessageHandler {
    async fn handle_message(&mut self, from: Uuid, message: Message) -> Result<(), NetworkError> {
        info!("📥 Узел {} получил сообщение от {}: тип={:?}, id={}", 
              self.node_id, from, message.message_type, message.id);
        Ok(())
    }
    
    async fn handle_peer_connected(&mut self, peer_info: PeerInfo) -> Result<(), NetworkError> {
        info!("🔗 Узел {} обнаружил новый узел: {} ({})", 
              self.node_id, peer_info.name, peer_info.id);
        Ok(())
    }
    
    async fn handle_peer_disconnected(&mut self, peer_id: Uuid) -> Result<(), NetworkError> {
        info!("🔌 Узел {} потерял соединение с узлом {}", self.node_id, peer_id);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализируем логирование
    env_logger::init();
    
    println!("🚀 TRIAD Testnet - Автоматическое обнаружение узлов");
    println!("   📋 План: UDP Discovery + TCP соединения");
    println!("   💡 Запустите на разных компьютерах для тестирования");
    
    // Параметры тестнета
    let discovery_port = 23456; // UDP порт для обнаружения
    let tcp_port = 8080;        // TCP порт для соединений
    
    // Создаем узел тестнета
    let mut node = TestnetNode::new(discovery_port, tcp_port).await?;
    
    // Инициализируем тестнет
    if let Err(e) = node.initialize_testnet().await {
        eprintln!("❌ Ошибка инициализации тестнета: {}", e);
        return Ok(());
    }
    
    println!("\n🎯 Тестнет готов! Нажмите Ctrl+C для остановки...");
    println!("   🌐 Для подключения других узлов запустите на других компьютерах:");
    println!("      cargo run --example Testnet");
    
    // Периодически отправляем тестовые сообщения
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = node.send_test_message().await {
                    eprintln!("⚠️  Ошибка отправки тестового сообщения: {}", e);
                }
            }
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }
    
    // Останавливаем тестнет
    if let Err(e) = node.shutdown().await {
        eprintln!("⚠️  Ошибка остановки тестнета: {}", e);
    }
    
    Ok(())
}
