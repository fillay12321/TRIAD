use triad::quantum::consensus::{ConsensusNode, ConsensusMessage, process_messages_parallel, check_interference_parallel};
use triad::sharding::assign_shard;
use std::time::{Instant, Duration};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

struct EnergyMetrics {
    total_operations: AtomicU64,
    quantum_energy: Vec<f64>,
    classical_energy: Vec<f64>,
    operation_times: Vec<Duration>,
}

impl EnergyMetrics {
    fn new() -> Self {
        Self {
            total_operations: AtomicU64::new(0),
            quantum_energy: Vec::new(),
            classical_energy: Vec::new(),
            operation_times: Vec::new(),
        }
    }

    fn record_operation(&mut self, duration: Duration, quantum_energy: f64, classical_energy: f64) {
        self.total_operations.fetch_add(1, Ordering::SeqCst);
        self.quantum_energy.push(quantum_energy);
        self.classical_energy.push(classical_energy);
        self.operation_times.push(duration);
    }

    fn calculate_energy_efficiency(&self) -> HashMap<String, f64> {
        let mut efficiency = HashMap::new();
        
        let total_ops = self.total_operations.load(Ordering::SeqCst) as f64;
        
        // Расчет среднего энергопотребления
        let avg_quantum_energy = self.quantum_energy.iter().sum::<f64>() / total_ops;
        let avg_classical_energy = self.classical_energy.iter().sum::<f64>() / total_ops;
        
        // Расчет энергоэффективности
        let energy_savings = ((avg_classical_energy - avg_quantum_energy) / avg_classical_energy) * 100.0;
        
        // Расчет энергопотребления на операцию
        let quantum_per_op = avg_quantum_energy / total_ops;
        let classical_per_op = avg_classical_energy / total_ops;
        
        efficiency.insert("avg_quantum_energy".to_string(), avg_quantum_energy);
        efficiency.insert("avg_classical_energy".to_string(), avg_classical_energy);
        efficiency.insert("energy_savings_percent".to_string(), energy_savings);
        efficiency.insert("quantum_energy_per_op".to_string(), quantum_per_op);
        efficiency.insert("classical_energy_per_op".to_string(), classical_per_op);
        
        efficiency
    }
}

// Функция для оценки энергопотребления квантовой операции
fn estimate_quantum_energy(duration: Duration, node_count: usize) -> f64 {
    // Базовое энергопотребление на интерференцию
    let base_energy = 0.001; // 1 мВт
    // Дополнительное энергопотребление на узел
    let per_node_energy = 0.0001; // 0.1 мВт
    // Время в секундах
    let time = duration.as_secs_f64();
    
    (base_energy + (per_node_energy * node_count as f64)) * time
}

// Функция для оценки энергопотребления классической операции
fn estimate_classical_energy(duration: Duration, node_count: usize) -> f64 {
    // Базовое энергопотребление на майнинг
    let base_energy = 100.0; // 100 Вт
    // Дополнительное энергопотребление на узел
    let per_node_energy = 10.0; // 10 Вт
    // Время в секундах
    let time = duration.as_secs_f64();
    
    (base_energy + (per_node_energy * node_count as f64)) * time
}

fn main() {
    println!("Анализ энергопотребления квантово-вдохновленной модели");
    
    let node_counts = vec![10, 100, 1000];
    let operations_per_test = 1000;
    let mut metrics = EnergyMetrics::new();
    
    let num_shards = 4;
    for count in node_counts {
        println!("\nТестирование энергопотребления для {} узлов:", count);
        
        let mut nodes: Vec<ConsensusNode> = (0..count)
            .map(|i| {
                let node_id = format!("node{}", i);
                let shard_id = assign_shard(&node_id, num_shards);
                ConsensusNode::new(node_id, shard_id)
            })
            .collect();
        
        for op in 0..operations_per_test {
            let message = ConsensusMessage {
                sender_id: format!("node{}", op % count),
                state_id: format!("state{}", op),
                raw_data: vec![op as u8, (op + 1) as u8, (op + 2) as u8],
            };
            
            let start = Instant::now();
            
            // Параллельная обработка сообщений
            process_messages_parallel(&mut nodes, message.clone()).unwrap();
            
            // Параллельная проверка консенсуса
            let consensus_reached = check_interference_parallel(&nodes, &format!("state{}", op)).unwrap();
            
            let duration = start.elapsed();
            
            // Оцениваем энергопотребление
            let quantum_energy = estimate_quantum_energy(duration, count);
            let classical_energy = estimate_classical_energy(duration, count);
            
            metrics.record_operation(duration, quantum_energy, classical_energy);
            
            if op % 100 == 0 {
                println!("Выполнено операций: {}/{}", op, operations_per_test);
            }
        }
        
        let efficiency = metrics.calculate_energy_efficiency();
        println!("\nРезультаты энергоэффективности для {} узлов:", count);
        println!("Среднее энергопотребление (квантовое): {:.6} Вт", efficiency["avg_quantum_energy"]);
        println!("Среднее энергопотребление (классическое): {:.6} Вт", efficiency["avg_classical_energy"]);
        println!("Экономия энергии: {:.2}%", efficiency["energy_savings_percent"]);
        println!("Энергопотребление на операцию (квантовое): {:.6} Вт", efficiency["quantum_energy_per_op"]);
        println!("Энергопотребление на операцию (классическое): {:.6} Вт", efficiency["classical_energy_per_op"]);
        
        // Расчет годового энергопотребления
        let yearly_quantum = efficiency["avg_quantum_energy"] * 365.0 * 24.0 * 3600.0;
        let yearly_classical = efficiency["avg_classical_energy"] * 365.0 * 24.0 * 3600.0;
        
        println!("\nГодовое энергопотребление:");
        println!("Квантово-вдохновленная модель: {:.2} кВт⋅ч", yearly_quantum / 1000.0);
        println!("Классический блокчейн: {:.2} кВт⋅ч", yearly_classical / 1000.0);
    }
} 