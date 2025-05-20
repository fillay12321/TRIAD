use triad::quantum::consensus::{ConsensusNode, ConsensusMessage};
use triad::sharding::assign_shard;
use std::time::Instant;
use std::sync::atomic::{AtomicU64, Ordering};
use rayon::prelude::*;

struct PerformanceMetrics {
    total_operations: AtomicU64,
    successful_operations: AtomicU64,
    failed_operations: AtomicU64,
}

impl PerformanceMetrics {
    fn new() -> Self {
        Self {
            total_operations: AtomicU64::new(0),
            successful_operations: AtomicU64::new(0),
            failed_operations: AtomicU64::new(0),
        }
    }

    fn record_operation(&self, success: bool) {
        self.total_operations.fetch_add(1, Ordering::SeqCst);
        if success {
            self.successful_operations.fetch_add(1, Ordering::SeqCst);
        } else {
            self.failed_operations.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn print_stats(&self, count: usize, duration: std::time::Duration) {
        let total = self.total_operations.load(Ordering::SeqCst);
        let successful = self.successful_operations.load(Ordering::SeqCst);
        let failed = self.failed_operations.load(Ordering::SeqCst);
        println!("\nРезультаты для {} узлов:", count);
        println!("Всего операций: {}", total);
        println!("Успешных операций: {} ({:.2}%)", successful, (successful as f64 / total as f64) * 100.0);
        println!("Неуспешных операций: {} ({:.2}%)", failed, (failed as f64 / total as f64) * 100.0);
        println!("Общее время обработки: {:?}", duration);
        println!("Среднее время на операцию: {:.2?}", duration / total as u32);
        let tps = total as f64 / duration.as_secs_f64();
        println!("Пропускная способность: {:.2} TPS", tps);
    }
}

fn main() {
    println!("Тест производительности квантово-вдохновленной модели (честный параллелизм)");
    
    let node_counts = vec![10, 100, 1000];
    let operations_per_node = 10;
    
    let num_shards = 4;
    for count in node_counts {
        println!("\nТестирование с {} узлами:", count);
        println!("Создаём узлы...");
        let metrics = PerformanceMetrics::new();
        // Создаём узлы
        let mut nodes: Vec<ConsensusNode> = (0..count)
            .map(|i| {
                let node_id = format!("node{}", i);
                let shard_id = assign_shard(&node_id, num_shards);
                ConsensusNode::new(node_id, shard_id)
            })
            .collect();
        
        println!("Подготавливаем сообщения...");
        // Для каждого узла — свой список сообщений
        let all_messages: Vec<Vec<ConsensusMessage>> = (0..count)
            .map(|node_idx| {
                (0..operations_per_node)
                    .map(|op| ConsensusMessage {
                        sender_id: format!("node{}", node_idx),
                        state_id: format!("state{}_{}", node_idx, op),
                        raw_data: vec![op as u8, (op + 1) as u8, (op + 2) as u8],
                    })
                    .collect()
            })
            .collect();
        
        println!("Начинаем обработку сообщений...");
        let start = Instant::now();
        // Параллельная обработка: каждый узел обрабатывает свою очередь сообщений
        nodes
            .par_iter_mut()
            .enumerate()
            .for_each(|(node_idx, node)| {
                println!("Обработка узла {}...", node_idx);
                for message in &all_messages[node_idx] {
                    let state = node.process_message(message.clone()).unwrap();
                    // Успех — если вероятность > 0.5 (Processed)
                    let success = state.superposition[0].probability > 0.5;
                    metrics.record_operation(success);
                }
            });
        let duration = start.elapsed();
        println!("Обработка завершена, выводим статистику...");
        metrics.print_stats(count, duration);
    }
} 