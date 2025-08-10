use triad::network::{Network, QuantumNetworkHandler, QuantumNetworkStats};
use triad::quantum::consensus::ConsensusNode;
use triad::transaction::{Transaction, SmartContract};
use triad::quantum::consensus::ConsensusMessage;
use triad::sharding::ShardEvent;
use triad::error_analysis::ErrorContext;
use triad::semantic::SemanticAction;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use std::time::{Duration, Instant};
use log::{info, debug, error};
use futures::future::join_all;

const NODES: usize = 5;
const SIMULATION_DURATION: Duration = Duration::from_secs(30);
const MESSAGE_INTERVAL: Duration = Duration::from_millis(500);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    info!("🚀 Запуск демонстрации квантовой сети");

    // Создаем узлы и обработчики
    let mut node_handles = Vec::new();
    let mut handler_vec = Vec::new();
    let mut network_vec = Vec::new();

    for i in 0..NODES {
        let node_name = format!("quantum_node_{}", i);
        let port = 8000 + i as u16;
        let consensus_node = Arc::new(Mutex::new(ConsensusNode::new(node_name.clone(), i)));
        let quantum_handler = QuantumNetworkHandler::new(Uuid::new_v4(), consensus_node.clone());
        let network = Network::new(node_name, port).await?;
        network_vec.push(network);
        handler_vec.push(quantum_handler);
    }
    info!("✅ Создано {} квантовых сетевых узлов", NODES);

    // Arc<Mutex<Vec<Network>>> и Arc<Vec<QuantumNetworkHandler>>
    let network_nodes = Arc::new(Mutex::new(network_vec));
    let quantum_handlers = Arc::new(handler_vec);

    // Запускаем обработчики событий параллельно
    for i in 0..NODES {
        let mut network_nodes = network_nodes.clone();
        let handler = quantum_handlers[i].clone();
        node_handles.push(tokio::spawn(async move {
            let mut nodes = network_nodes.lock().await;
            if let Err(e) = nodes[i].start().await {
                error!("Ошибка запуска узла {}: {}", i, e);
            }
            if let Err(e) = nodes[i].run_event_loop(handler).await {
                error!("Ошибка в event loop узла {}: {}", i, e);
            }
        }));
    }
    info!("🔄 Обработчики событий запущены");

    // Генерация и отправка сообщений параллельно
    let network_nodes_msg = network_nodes.clone();
    let msg_handle = tokio::spawn(async move {
        let start_time = Instant::now();
        let mut message_count = 0;
        while start_time.elapsed() < SIMULATION_DURATION {
            let mut tasks = Vec::new();
            for i in 0..NODES {
                let network_nodes = network_nodes_msg.clone();
                tasks.push(tokio::spawn(async move {
                    let mut nodes = network_nodes.lock().await;
                    for j in 0..NODES {
                        if i != j {
                            let message_type = get_random_message_type();
                            let payload = generate_random_payload();
                            if let Err(e) = nodes[i].send_to_peer(
                                nodes[j].node_id(),
                                message_type,
                                payload,
                            ).await {
                                error!("Ошибка отправки сообщения от {} к {}: {}", i, j, e);
                            }
                        }
                    }
                }));
            }
            join_all(tasks).await;
            message_count += NODES * (NODES - 1);
            tokio::time::sleep(MESSAGE_INTERVAL).await;
        }
        info!("📈 Всего отправлено сообщений: {}", message_count);
    });

    // Параллельный вывод статистики
    let quantum_handlers_stats = quantum_handlers.clone();
    let stats_handle = tokio::spawn(async move {
        let mut last_stats = Instant::now();
        while last_stats.elapsed() < SIMULATION_DURATION + Duration::from_secs(2) {
            print_network_stats(&quantum_handlers_stats).await;
            tokio::time::sleep(Duration::from_secs(5)).await;
            last_stats = Instant::now();
        }
    });

    // Дожидаемся завершения всех задач
    let _ = join_all(node_handles).await;
    let _ = msg_handle.await;
    let _ = stats_handle.await;
    info!("✅ Демонстрация квантовой сети завершена");
    Ok(())
}

fn get_random_message_type() -> triad::network::MessageType {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    match rng.gen_range(0..6) {
        0 => {
            let transaction = Transaction {
                id: Uuid::new_v4(),
                sender: format!("user_{}", rng.gen_range(0..100)),
                receiver: format!("user_{}", rng.gen_range(0..100)),
                amount: rng.gen_range(1.0..1000.0),
                data: vec![rng.gen()],
                status: triad::transaction::TransactionStatus::Pending,
                timestamp: std::time::SystemTime::now(),
            };
            triad::network::MessageType::Transaction(transaction)
        },
        1 => {
            let contract = SmartContract {
                id: Uuid::new_v4(),
                creator: format!("creator_{}", rng.gen_range(0..10)),
                code: vec![rng.gen()],
                state: vec![rng.gen()],
                status: triad::transaction::ContractStatus::Deployed,
                timestamp: std::time::SystemTime::now(),
            };
            triad::network::MessageType::SmartContract(contract)
        },
        2 => {
            let consensus_msg = ConsensusMessage {
                sender_id: format!("node_{}", rng.gen_range(0..NODES)),
                state_id: format!("state_{}", rng.gen_range(0..100)),
                raw_data: vec![rng.gen()],
            };
            triad::network::MessageType::Consensus(consensus_msg)
        },
        3 => {
            let shard_event = ShardEvent {
                shard_id: format!("shard_{}", rng.gen_range(0..10)),
                event_type: "NodeJoined".to_string(),
                node_id: Uuid::new_v4().to_string(),
                timestamp: std::time::SystemTime::now(),
                details: Some(format!("Node {} joined shard", rng.gen_range(0..100))),
            };
            triad::network::MessageType::ShardEvent(shard_event)
        },
        4 => {
            let error_ctx = ErrorContext {
                timestamp: std::time::SystemTime::now(),
                component_id: Some(Uuid::new_v4()),
                related_component_id: Some(Uuid::new_v4()),
                category: triad::error_analysis::ErrorCategory::NetworkConnection,
                severity: triad::error_analysis::ErrorSeverity::Low,
                description: "Test error".to_string(),
                stack_trace: None,
                system_state: std::collections::HashMap::new(),
                quantum_state: None,
                shard_info: None,
                consensus_data: None,
                error_type: triad::error_analysis::ErrorType::SystemError {
                    component: "network".to_string(),
                    error_code: "TEST_001".to_string(),
                    details: "Test network error".to_string(),
                },
                affected_components: vec!["network".to_string()],
                propagation_path: vec!["network".to_string()],
            };
            triad::network::MessageType::ErrorEvent(error_ctx)
        },
        5 => {
            let semantic_action = SemanticAction {
                entity_id: format!("network_{}", rng.gen_range(0..10)),
                action_type: "OptimizeNetwork".to_string(),
                details: Some(format!("Optimize network with latency={}ms, bandwidth={:.2}, connectivity={:.2}", 
                    rng.gen_range(10..100), rng.gen_range(0.1..1.0), rng.gen_range(0.5..1.0))),
            };
            triad::network::MessageType::SemanticAction(semantic_action)
        },
        _ => triad::network::MessageType::Error("Unknown type".to_string()),
    }
}

fn generate_random_payload() -> Vec<u8> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(10..100);
    (0..size).map(|_| rng.gen()).collect()
}

async fn print_network_stats(handlers: &[QuantumNetworkHandler]) {
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("📊 Статистика квантовой сети:");
    for (i, handler) in handlers.iter().enumerate() {
        let stats = handler.get_quantum_network_stats().await;
        info!("Узел {}: волны={}, интерференции={}, кэш={}, амплитуда={:.2}",
            i, stats.active_waves, stats.interference_events, 
            stats.cached_messages, stats.total_wave_amplitude);
    }
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
} 