use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;
use uuid::Uuid;
use log::{info, error, warn};

// Импортируем наши модули
use triad::network::{
    Network, NetworkError, NetworkEvent, MessageType, 
    types::PeerInfo, discovery::DiscoveryService, transport::TransportService
};

#[derive(Debug)]
pub struct TestnetNode {
    pub network: Network,
    pub discovery_port: u16,
    pub tcp_port: u16,
}

impl TestnetNode {
    pub fn new(discovery_port: u16, tcp_port: u16) -> Result<Self, NetworkError> {
        let node_name = format!("testnet-node-{}", Uuid::new_v4().to_string()[..8].to_string());
        
        // Создаем основную сеть с discovery и transport сервисами
        let network = Network::new(node_name, tcp_port).await?;
        
        Ok(Self {
            network,
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
        self.network.start().await?;
        println!("   ✅ Сетевые сервисы запущены");
        
        // 2. Запускаем event loop для обработки сетевых событий
        println!("   🔄 Запуск event loop...");
        let event_sender = self.network.event_sender.clone();
        
        // Запускаем event loop в отдельной задаче
        tokio::spawn(async move {
            let handler = TestnetMessageHandler;
            if let Err(e) = handler.run_event_loop(event_sender).await {
                error!("Event loop error: {}", e);
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
        self.network.stop().await?;
        
        println!("✅ Тестнет остановлен");
        Ok(())
    }
}

/// Обработчик сообщений для тестнета
struct TestnetMessageHandler;

impl triad::network::MessageHandler for TestnetMessageHandler {
    async fn handle_message(&self, event: NetworkEvent) -> Result<(), NetworkError> {
        match event {
            NetworkEvent::MessageReceived { from, message } => {
                info!("📥 Получено сообщение от {}: {:?}", from, message.message_type);
                Ok(())
            }
            NetworkEvent::PeerConnected(peer) => {
                info!("🔗 Подключен новый пир: {} ({})", peer.name, peer.address);
                Ok(())
            }
            NetworkEvent::PeerDisconnected(peer_id) => {
                info!("🔌 Отключен пир: {}", peer_id);
                Ok(())
            }
        }
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
    let mut node = TestnetNode::new(discovery_port, tcp_port)?;
    
    // Инициализируем тестнет
    if let Err(e) = node.initialize_testnet().await {
        eprintln!("❌ Ошибка инициализации тестнета: {}", e);
        return Ok(());
    }
    
    println!("\n🎯 Тестнет готов! Нажмите Ctrl+C для остановки...");
    println!("   🌐 Для подключения других узлов запустите на других компьютерах:");
    println!("      cargo run --example demo_work");
    
    // Периодически отправляем тестовые сообщения
    let node_ref = &node;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(e) = node_ref.send_test_message().await {
                eprintln!("⚠️  Ошибка отправки тестового сообщения: {}", e);
            }
        }
    });
    
    // Ждем сигнала остановки
    tokio::signal::ctrl_c().await?;
    
    // Останавливаем тестнет
    if let Err(e) = node.shutdown().await {
        eprintln!("⚠️  Ошибка остановки тестнета: {}", e);
    }
    
    Ok(())
}
