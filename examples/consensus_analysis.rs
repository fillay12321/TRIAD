use triad::quantum::consensus::{ConsensusNode, ConsensusMessage, process_messages_parallel, check_interference_parallel};
use triad::sharding::assign_shard;
use std::time::{Instant, Duration};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

struct ConsensusMetrics {
    total_messages: AtomicU64,
    consensus_reached: AtomicU64,
    consensus_failed: AtomicU64,
    message_latency: Vec<Duration>,
    interference_patterns: HashMap<String, u64>,
}

impl ConsensusMetrics {
    fn new() -> Self {
        Self {
            total_messages: AtomicU64::new(0),
            consensus_reached: AtomicU64::new(0),
            consensus_failed: AtomicU64::new(0),
            message_latency: Vec::new(),
            interference_patterns: HashMap::new(),
        }
    }

    fn record_consensus(&mut self, duration: Duration, reached: bool, pattern: String) {
        self.total_messages.fetch_add(1, Ordering::SeqCst);
        if reached {
            self.consensus_reached.fetch_add(1, Ordering::SeqCst);
        } else {
            self.consensus_failed.fetch_add(1, Ordering::SeqCst);
        }
        self.message_latency.push(duration);
        *self.interference_patterns.entry(pattern).or_insert(0) += 1;
    }

    fn analyze(&self) -> HashMap<String, f64> {
        let mut analysis = HashMap::new();
        
        let total = self.total_messages.load(Ordering::SeqCst) as f64;
        let reached = self.consensus_reached.load(Ordering::SeqCst) as f64;
        let failed = self.consensus_failed.load(Ordering::SeqCst) as f64;

        // Базовые метрики консенсуса
        analysis.insert("total_messages".to_string(), total);
        analysis.insert("consensus_success_rate".to_string(), (reached / total) * 100.0);
        analysis.insert("consensus_failure_rate".to_string(), (failed / total) * 100.0);

        // Анализ латентности
        if !self.message_latency.is_empty() {
            let latencies: Vec<f64> = self.message_latency.iter()
                .map(|d| d.as_nanos() as f64)
                .collect();
            
            let avg_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
            let min_latency = latencies.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_latency = latencies.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            
            analysis.insert("avg_latency_ns".to_string(), avg_latency);
            analysis.insert("min_latency_ns".to_string(), min_latency);
            analysis.insert("max_latency_ns".to_string(), max_latency);
        }

        // Анализ паттернов интерференции
        let total_patterns: u64 = self.interference_patterns.values().sum();
        for (pattern, count) in &self.interference_patterns {
            let frequency = (*count as f64 / total_patterns as f64) * 100.0;
            analysis.insert(format!("pattern_{}_frequency", pattern), frequency);
        }

        analysis
    }
}

fn main() {
    println!("Анализ консенсуса в квантово-вдохновленной модели");
    
    let node_counts = vec![10, 100, 1000];
    let messages_per_test = 1000;
    let mut metrics = ConsensusMetrics::new();
    
    let num_shards = 4;
    for count in node_counts {
        println!("\nАнализ консенсуса для {} узлов:", count);
        
        let mut nodes: Vec<ConsensusNode> = (0..count)
            .map(|i| {
                let node_id = format!("node{}", i);
                let shard_id = assign_shard(&node_id, num_shards);
                ConsensusNode::new(node_id, shard_id)
            })
            .collect();
        
        for msg in 0..messages_per_test {
            let message = ConsensusMessage {
                sender_id: format!("node{}", msg % count),
                state_id: format!("state{}", msg),
                raw_data: vec![msg as u8, (msg + 1) as u8, (msg + 2) as u8],
            };
            
            let start = Instant::now();
            
            // Параллельная обработка сообщений
            process_messages_parallel(&mut nodes, message.clone()).unwrap();
            
            // Параллельная проверка консенсуса и сбор паттернов интерференции
            let consensus_reached = check_interference_parallel(&nodes, &format!("state{}", msg)).unwrap();
            let interference_pattern = if consensus_reached { "constructive" } else { "destructive" }.to_string();
            
            let duration = start.elapsed();
            metrics.record_consensus(duration, consensus_reached, interference_pattern);
            
            if msg % 100 == 0 {
                println!("Обработано сообщений: {}/{}", msg, messages_per_test);
            }
        }
        
        let analysis = metrics.analyze();
        println!("\nРезультаты анализа для {} узлов:", count);
        println!("Всего сообщений: {}", analysis["total_messages"]);
        println!("Успешных консенсусов: {:.2}%", analysis["consensus_success_rate"]);
        println!("Средняя латентность: {:.2} нс", analysis["avg_latency_ns"]);
        println!("Минимальная латентность: {:.2} нс", analysis["min_latency_ns"]);
        println!("Максимальная латентность: {:.2} нс", analysis["max_latency_ns"]);
        
        // Анализ паттернов интерференции
        println!("\nПаттерны интерференции:");
        println!("Конструктивная интерференция: {:.2}%", 
            analysis.get("pattern_constructive_frequency").unwrap_or(&0.0));
        println!("Деструктивная интерференция: {:.2}%", 
            analysis.get("pattern_destructive_frequency").unwrap_or(&0.0));
    }
} 