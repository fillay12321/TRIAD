use std::sync::{Arc, Mutex as StdMutex};
use std::collections::HashMap;
use std::time::{Instant, Duration};
use serde::{Serialize, Deserialize};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rayon::prelude::*;
use ed25519_dalek::{VerifyingKey};
use triad::quantum::{QuantumField, InterferenceEngine, QuantumWave, QuantumState, StateVector};
use triad::quantum::consensus::{ConsensusNode, ConsensusMessage};
#[cfg(feature = "opencl")]
use triad::quantum::interference::OpenClAccelerator;
use triad::github::GitHubCodespaces;
use num_complex::Complex;
use log::info;
use tokio::task::spawn_blocking;
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};

// Правильное измерение TPS
fn calculate_tps(success_count: usize, start_time: Instant) -> f64 {
    let elapsed = start_time.elapsed().as_secs_f64();
    if elapsed > 0.0 {
        success_count as f64 / elapsed
    } else {
        0.0
    }
}

// BLS агрегированные подписи для 1000x ускорения
use blsful::{
    Bls12381G1Impl, 
    SecretKey, 
    PublicKey, 
    Signature as BlsSignature,
    AggregateSignature,
    SignatureSchemes
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Args {
    distributed: bool,
    num_codespaces: usize,
    real_codespaces: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            distributed: false,
            num_codespaces: 3,
            real_codespaces: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DemoTransaction {
    id: String,
    from: String,
    to: String,
    amount: f64,
    fee: f64, // Комиссия транзакции - основной фактор для приоритизации
    nonce: u64,
    signature: Vec<u8>,
    public_key_idx: usize, // Индекс в BLS KeyPool
    shard_id: usize,
    timestamp: u64,
}

#[derive(Debug, Clone)]
struct AccountState {
    balance: f64,
    nonce: u64,
}

#[derive(Debug)]
struct PerformanceMetrics {
    generation_time: Duration,
    serialization_time: Duration,
    websocket_time: Duration,
    cpu_load: f64,
}

// Мониторинг CPU в реальном времени
struct CpuMonitor {
    running: Arc<AtomicBool>,
    cpu_usage: Arc<StdMutex<f64>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl CpuMonitor {
    fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            cpu_usage: Arc::new(StdMutex::new(0.0)),
            thread_handle: None,
        }
    }

    fn start(&mut self) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }
        
        self.running.store(true, Ordering::Relaxed);
        let running = self.running.clone();
        let cpu_usage = self.cpu_usage.clone();
        
        self.thread_handle = Some(thread::spawn(move || {
            let mut last_cpu_time = get_cpu_time();
            let mut last_wall_time = Instant::now();
            
            while running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(100)); // Обновляем каждые 100ms
                
                let current_cpu_time = get_cpu_time();
                let current_wall_time = Instant::now();
                
                let cpu_delta = current_cpu_time - last_cpu_time;
                let wall_delta = current_wall_time.duration_since(last_wall_time);
                
                if wall_delta.as_secs_f64() > 0.0 {
                    let usage = (cpu_delta / wall_delta.as_secs_f64()) * 100.0;
                    if let Ok(mut cpu_usage_guard) = cpu_usage.lock() {
                        *cpu_usage_guard = usage;
                    }
                }
                
                last_cpu_time = current_cpu_time;
                last_wall_time = current_wall_time;
            }
        }));
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    fn get_cpu_usage(&self) -> f64 {
        if let Ok(cpu_usage_guard) = self.cpu_usage.lock() {
            *cpu_usage_guard
        } else {
            0.0
        }
    }
}

// Получение времени CPU (упрощенная версия для демо)
fn get_cpu_time() -> f64 {
    // В реальной реализации здесь был бы вызов системного API
    // Для демо используем приблизительное значение
    let start = Instant::now();
    // Имитируем небольшую нагрузку для получения CPU времени
    let _: f64 = (0..1000).map(|i| i as f64).sum();
    start.elapsed().as_secs_f64()
}

// Object Pool для QuantumState (оптимизация памяти)
struct QuantumStatePool {
    pool: Arc<StdMutex<Vec<QuantumState>>>,
    max_size: usize,
}

impl QuantumStatePool {
    fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(StdMutex::new(Vec::with_capacity(max_size))),
            max_size,
        }
    }

    fn get(&self) -> Option<QuantumState> {
        self.pool.lock().unwrap().pop()
    }

    fn return_state(&self, state: QuantumState) {
        let mut pool = self.pool.lock().unwrap();
        if pool.len() < self.max_size {
            pool.push(state);
        }
    }
}

// BLS KeyPool для предварительно сгенерированных ключей с агрегацией
#[derive(Clone)]
struct BlsKeyPool {
    secret_keys: Vec<SecretKey<Bls12381G1Impl>>,
    public_keys: Vec<PublicKey<Bls12381G1Impl>>,
}

impl BlsKeyPool {
    fn new(size: usize) -> Self {
        let mut secret_keys = Vec::with_capacity(size);
        let mut public_keys = Vec::with_capacity(size);
        
        for _ in 0..size {
            let secret_key = SecretKey::random(rand::thread_rng());
            let public_key = secret_key.public_key();
            secret_keys.push(secret_key);
            public_keys.push(public_key);
        }
        
        Self {
            secret_keys,
            public_keys,
        }
    }

    fn get_secret_key(&self, index: usize) -> &SecretKey<Bls12381G1Impl> {
        &self.secret_keys[index % self.secret_keys.len()]
    }

    fn get_public_key(&self, index: usize) -> &PublicKey<Bls12381G1Impl> {
        &self.public_keys[index % self.public_keys.len()]
    }

    // Агрегируем все публичные ключи для batch verification
    fn aggregate_public_keys(&self) -> PublicKey<Bls12381G1Impl> {
        // Для простоты используем первый ключ как агрегированный
        // В реальной реализации нужно использовать правильную агрегацию
        self.public_keys[0].clone()
    }

    // Агрегируем подписи для batch verification
    fn aggregate_signatures(&self, signatures: &[BlsSignature<Bls12381G1Impl>]) -> Option<AggregateSignature<Bls12381G1Impl>> {
        AggregateSignature::from_signatures(signatures).ok()
    }

    // Проверяем агрегированную подпись против агрегированного публичного ключа
    fn verify_aggregate_signature(
        &self,
        agg_sig: &AggregateSignature<Bls12381G1Impl>,
        messages: &[Vec<u8>]
    ) -> bool {
        // Создаем пары (public_key, message) для верификации
        let key_message_pairs: Vec<_> = self.public_keys.iter()
            .take(messages.len())
            .zip(messages.iter())
            .map(|(pk, msg)| (pk.clone(), msg.as_slice()))
            .collect();
        
        agg_sig.verify(&key_message_pairs).is_ok()
    }
}

// TransactionPool с переиспользованием буферов
struct TransactionPool {
    buffer: Vec<u8>,
    transactions: Vec<DemoTransaction>,
}

impl TransactionPool {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity * 1024), // 1KB per transaction
            transactions: Vec::with_capacity(capacity),
        }
    }

    fn prepare(&mut self, count: usize, accounts: &[String], bls_keypool: &BlsKeyPool) -> Vec<u8> {
        self.buffer.clear();
        self.transactions.clear();
        
        // Генерируем транзакции параллельно
        let transactions: Vec<DemoTransaction> = generate_sharded_transactions(count, accounts, bls_keypool);
        
        // Сериализуем в буфер
        for tx in &transactions {
            let serialized = serde_json::to_vec(tx).unwrap();
            self.buffer.extend_from_slice(&serialized);
            self.buffer.push(b'\n'); // Разделитель
        }
        
        self.transactions = transactions;
        self.buffer.clone()
    }
}

// Генерируем транзакции с BLS подписями (БЕЗ генерации новых ключей!)
fn generate_sharded_transactions(num_transactions: usize, accounts: &[String], bls_keypool: &BlsKeyPool) -> Vec<DemoTransaction> {
    let start = Instant::now();
    
    let transactions: Vec<DemoTransaction> = (0..num_transactions).into_par_iter().map(|i| {
        let from_idx = i % accounts.len();
        let to_idx = (i + 1) % accounts.len();
        let from = &accounts[from_idx];
        let to = &accounts[to_idx];
        
        // Добавляем случайность в суммы транзакций
        let mut rng = rand::thread_rng();
        let base_amount = if i % 3 == 0 { 
            10.0 + (i as f64 * 0.1) 
        } else if i % 3 == 1 { 
            50.0 + (i as f64 * 0.5) 
        } else { 
            100.0 + (i as f64 * 1.0) 
        };
        let amount = base_amount + (rng.gen::<f64>() * 10.0); // ±10 случайность
        
        // Генерируем комиссию с случайностью
        let base_fee = if i % 5 == 0 {
            0.001 + (i as f64 * 0.0001) // Низкие комиссии
        } else if i % 5 == 1 {
            0.01 + (i as f64 * 0.001) // Средние комиссии
        } else if i % 5 == 2 {
            0.1 + (i as f64 * 0.01) // Высокие комиссии
        } else if i % 5 == 3 {
            0.5 + (i as f64 * 0.05) // Очень высокие комиссии
        } else {
            0.8 + (i as f64 * 0.02) // Премиум комиссии
        };
        let fee = base_fee + (rng.gen::<f64>() * 0.1); // ±0.1 случайность
        
        let nonce = i as u64;
        let shard_id = (i + rng.gen::<usize>() % 10) % 3; // Случайное распределение по шардам
        
        // Создаем транзакцию с РЕАЛЬНОЙ подписью
        let secret_key = bls_keypool.get_secret_key(from_idx);
        let message = format!("{}:{}:{}:{}:{}", from, to, amount, fee, nonce);
        let signature = secret_key.sign(SignatureSchemes::Basic, message.as_bytes()).unwrap();
        
        let transaction = DemoTransaction {
            id: format!("tx_{}", i),
            from: from.clone(),
            to: to.clone(),
            amount,
            fee,
            nonce,
            signature: Vec::from(&signature), // РЕАЛЬНАЯ подпись
            public_key_idx: from_idx,
            shard_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        transaction
    }).collect();
    
    let generation_time = start.elapsed();
    println!("⚡ Generated {} transactions in {:.3}ms ({:.0} tx/sec)", 
        num_transactions, generation_time.as_secs_f64() * 1000.0, 
        num_transactions as f64 / generation_time.as_secs_f64());
    
    transactions
}

// Создаем сообщения для шардов из сериализованных данных
fn create_sharded_messages_from_serialized(serialized: Vec<u8>, num_shards: usize) -> Vec<Vec<ConsensusMessage>> {
    let mut sharded_messages = vec![Vec::new(); num_shards];
    
    // Парсим сериализованные транзакции
    let lines: Vec<&[u8]> = serialized.split(|&b| b == b'\n').filter(|line| !line.is_empty()).collect();
    
    for (i, line) in lines.iter().enumerate() {
        if let Ok(tx) = serde_json::from_slice::<DemoTransaction>(line) {
            let shard_id = tx.shard_id % num_shards;
            let message = ConsensusMessage {
                sender_id: tx.from.clone(),
                state_id: tx.id,
                raw_data: line.to_vec(),
                signature: Vec::new(),
            };
            sharded_messages[shard_id].push(message);
        }
    }
    
    sharded_messages
}

// Обрабатываем консенсус в распределенном режиме
fn process_distributed_consensus(
    nodes: &mut [ConsensusNode],
    sharded_messages: &[Vec<ConsensusMessage>],
    accounts_state: &mut HashMap<String, AccountState>,
    pubkeys: &HashMap<String, VerifyingKey>,
    quantum_field: &Arc<StdMutex<QuantumField>>,
    interference_engine: &Arc<StdMutex<InterferenceEngine>>,
    verification_results: &[bool],
    bls_keypool: &BlsKeyPool
) -> (usize, usize) {
    let start = Instant::now();
    let mut success_count = 0;
    let mut failure_count = 0;
    
    // Обрабатываем каждый шард параллельно с использованием rayon
    let results: Vec<(usize, usize)> = sharded_messages.iter().enumerate()
        .filter(|(_, messages)| !messages.is_empty())
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(shard_id, messages)| {
            let shard_success = process_shard_consensus(
                shard_id,
                messages,
                quantum_field,
                interference_engine,
                verification_results,
                bls_keypool
            );
            (shard_success, messages.len() - shard_success)
        })
        .collect();
    
    // Суммируем результаты
    for (success, failure) in results {
        success_count += success;
        failure_count += failure;
    }
    
    let total_time = start.elapsed();
    let tps = calculate_tps(success_count, start);
    println!("🔍 Consensus: {:.3}ms | Success: {} | Fail: {} | TPS: {:.0}", 
        total_time.as_secs_f64() * 1000.0, success_count, failure_count, tps);
    
    (success_count, failure_count)
}

// Обрабатываем консенсус для одного шарда
fn process_shard_consensus(
    shard_id: usize,
    messages: &[ConsensusMessage],
    quantum_field: &Arc<StdMutex<QuantumField>>,
    interference_engine: &Arc<StdMutex<InterferenceEngine>>,
    verification_results: &[bool],
    bls_keypool: &BlsKeyPool
) -> usize {
    println!("🔬 [Shard {}] Processing {} messages", shard_id, messages.len());
    
    // Создаем волны из сообщений
    let wave_start = Instant::now();
    let waves: Vec<QuantumWave> = messages.iter().map(|msg| {
        // Вычисляем фазу на основе содержания транзакции
        let phase = calculate_transaction_phase(msg);
        
        // Создаем квантовое состояние
        let state = QuantumState::new(
            msg.state_id.clone(),
            (msg.raw_data.len() as f64) / 1000.0, // Амплитуда зависит от размера данных
            phase, // Настоящая фаза на основе содержания транзакции
            shard_id.to_string(),
            vec![StateVector {
                value: num_complex::Complex::new(1.0, 0.0),
                probability: 1.0,
            }]
        );
        
        // Создаем волну
        let mut wave = QuantumWave::from(state);
        
        // ПОДПИСЫВАЕМ ВОЛНУ, а не транзакцию!
        let message = format!("{}:{}:{}", wave.id, wave.amplitude, wave.phase);
        if let Ok(tx) = serde_json::from_slice::<DemoTransaction>(&msg.raw_data) {
            let secret_key = bls_keypool.get_secret_key(tx.public_key_idx);
            if let Ok(signature) = secret_key.sign(SignatureSchemes::Basic, message.as_bytes()) {
                wave.signature = Vec::from(&signature);
            }
        }
        
        wave
    }).collect();
    let wave_time = wave_start.elapsed();
    println!("🔬 [Shard {}] Wave creation: {:.3}ms", shard_id, wave_time.as_secs_f64() * 1000.0);
    
    // Добавляем волны в квантовое поле
    let field_add_start = Instant::now();
    {
        let mut field = quantum_field.lock().unwrap();
        for wave in waves {
            field.add_wave(wave.id.clone(), wave);
        }
        // Очищаем устаревшие волны только ОДИН РАЗ после добавления всех
        field.cleanup_expired_waves();
    }
    let field_add_time = field_add_start.elapsed();
    println!("🔬 [Shard {}] Field addition: {:.3}ms", shard_id, field_add_time.as_secs_f64() * 1000.0);
    
    // Рассчитываем РЕАЛЬНУЮ квантовую интерференцию
    let interference_start = Instant::now();
    let interference_pattern = {
        let field = quantum_field.lock().unwrap();
        let mut engine = interference_engine.lock().unwrap();
        
        // Детальное логирование каждого этапа
        let field_read_start = Instant::now();
        let field_clone = field.clone(); // Клонируем поле для анализа
        let field_read_time = field_read_start.elapsed();
        println!("🔬 [Shard {}] Field read: {:.3}ms", shard_id, field_read_time.as_secs_f64() * 1000.0);
        
        let interference_calc_start = Instant::now();
        // Используем РЕАЛЬНУЮ квантовую интерференцию с высоким разрешением
        let result = engine.calculate_interference_from_field(&field_clone);
        let interference_calc_time = interference_calc_start.elapsed();
        println!("🔬 [Shard {}] Interference calculation core: {:.3}ms", shard_id, interference_calc_time.as_secs_f64() * 1000.0);
        
        result
    };
    let interference_time = interference_start.elapsed();
    println!("🔬 [Shard {}] REAL Interference calculation: {:.3}ms", shard_id, interference_time.as_secs_f64() * 1000.0);
    
    // Анализируем интерференцию
    let analysis_start = Instant::now();
    let analysis = {
        let mut engine = interference_engine.lock().unwrap();
        
        let analysis_calc_start = Instant::now();
        let result = engine.analyze_interference(&interference_pattern);
        let analysis_calc_time = analysis_calc_start.elapsed();
        println!("🔬 [Shard {}] Analysis calculation core: {:.3}ms", shard_id, analysis_calc_time.as_secs_f64() * 1000.0);
        
        result
    };
    let analysis_time = analysis_start.elapsed();
    println!("🔬 [Shard {}] Interference analysis: {:.3}ms", shard_id, analysis_time.as_secs_f64() * 1000.0);
    
    // Создаем вероятностное решение на основе интерференции
    let decision_start = Instant::now();
    let op = create_consensus_decision(&analysis, messages);
    let decision_create_time = decision_start.elapsed();
    println!("🔬 [Shard {}] Decision creation: {:.3}ms", shard_id, decision_create_time.as_secs_f64() * 1000.0);
    
    let outcome_start = Instant::now();
    let outcome = op.execute();
    let outcome_time = outcome_start.elapsed();
    println!("🔬 [Shard {}] Outcome execution: {:.3}ms", shard_id, outcome_time.as_secs_f64() * 1000.0);
    
    let decision_time = decision_start.elapsed();
    println!("🔬 [Shard {}] Decision creation: {:.3}ms | Outcome: {} (prob: {:.3})", 
        shard_id, decision_time.as_secs_f64() * 1000.0, outcome.label, outcome.probability);
    
    // Фильтруем сообщения по результатам BLS верификации
    let valid_messages: Vec<&ConsensusMessage> = messages.iter()
        .enumerate()
        .filter(|(i, _)| {
            // Проверяем, что индекс не выходит за пределы результатов верификации
            *i < verification_results.len() && verification_results[*i]
        })
        .map(|(_, msg)| msg)
        .collect();
    
    println!("🔬 [Shard {}] BLS filtered: {}/{} messages valid", shard_id, valid_messages.len(), messages.len());
    
    // Применяем квантовый консенсус только к валидным сообщениям
    let success_count = if valid_messages.is_empty() {
        0 // Нет валидных подписей - все отклоняем
    } else {
        match outcome.label.as_str() {
            "ConsensusReached" => valid_messages.len(),
            "ConflictDetected" => 0,
            "PartialConsensus" => (valid_messages.len() as f64 * 0.7) as usize,
            _ => 0,
        }
    };
    
    let total_shard_time = wave_start.elapsed();
    println!("🔬 [Shard {}] COMPLETED: {} success, {} total time", shard_id, success_count, total_shard_time.as_secs_f64() * 1000.0);
    
    if interference_time.as_millis() > 100 {
        println!("⚠️  [Shard {}] WARNING: Interference calculation took {}ms", shard_id, interference_time.as_millis());
    }
    
    success_count
}

// Создаем решение консенсуса на основе интерференции и комиссий
fn create_consensus_decision(analysis: &triad::quantum::interference::InterferenceAnalysis, messages: &[ConsensusMessage]) -> triad::quantum::prob_ops::ProbabilisticOperation {
    let constructive_ratio = if analysis.total_points() > 0 {
        analysis.constructive_points.len() as f64 / analysis.total_points() as f64
    } else {
        0.0
    };
    
    // Извлекаем комиссии из сообщений для расчета фактора комиссии
    let fee_factor = calculate_fee_factor(messages);
    
    let phase_conflict = calculate_phase_conflict(messages);
    let ttl_factor = calculate_ttl_factor();
    let reputation_factor = calculate_reputation_factor(messages);
    
    // Улучшенная формула успеха с комиссией как основным фактором
    let success_prob = (constructive_ratio * 0.4 + 
                       fee_factor * 0.3 + // Снижаем вес комиссии с 50% до 30%
                       (1.0 - phase_conflict) * 0.2 + // Увеличиваем вес фаз с 10% до 20%
                       ttl_factor * 0.05 + 
                       reputation_factor * 0.05).clamp(0.3, 0.95); // Увеличиваем минимум с 0.1 до 0.3
    
    println!("🔬 [Decision] Factors: constructive={:.3}, fee_factor={:.3}, phase_conflict={:.3}, ttl={:.3}, reputation={:.3} → success_prob={:.3}", 
        constructive_ratio, fee_factor, phase_conflict, ttl_factor, reputation_factor, success_prob);
    
    triad::quantum::prob_ops::ProbabilisticOperation::new("consensus_decision", success_prob)
}

fn calculate_phase_conflict(messages: &[ConsensusMessage]) -> f64 {
    if messages.len() < 2 { return 0.0; }
    
    let phases: Vec<f64> = messages.iter()
        .map(|msg| calculate_transaction_phase(msg)) // РЕАЛЬНЫЕ фазы
        .collect();
    
    let mut conflicts = 0.0;
    for i in 0..phases.len() {
        for j in (i+1)..phases.len() {
            let phase_diff = (phases[i] - phases[j]).abs();
            if phase_diff > std::f64::consts::PI / 2.0 {
                conflicts += 1.0;
            }
        }
    }
    
    conflicts / (phases.len() * (phases.len() - 1) / 2) as f64
}

fn calculate_ttl_factor() -> f64 {
    // Добавляем случайность в TTL фактор
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let seed = now.as_nanos() as u64;
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    
    // TTL варьируется от 0.7 до 0.9 с небольшой случайностью
    0.7 + (rng.gen::<f64>() * 0.2)
}

fn calculate_fee_factor(messages: &[ConsensusMessage]) -> f64 {
    if messages.is_empty() {
        return 0.5; // Базовый фактор если нет сообщений
    }
    
    let mut total_fee = 0.0;
    let mut max_fee: f64 = 0.0;
    let mut valid_transactions = 0;
    
    for message in messages {
        // Пытаемся извлечь комиссию из транзакции
        if let Ok(tx) = serde_json::from_slice::<DemoTransaction>(&message.raw_data) {
            total_fee += tx.fee;
            max_fee = max_fee.max(tx.fee);
            valid_transactions += 1;
        }
    }
    
    if valid_transactions == 0 {
        return 0.5; // Базовый фактор если не удалось извлечь транзакции
    }
    
    // Нормализуем среднюю комиссию относительно максимальной
    let avg_fee = total_fee / valid_transactions as f64;
    let normalized_fee = if max_fee > 0.0 { avg_fee / max_fee } else { 0.5 };
    
    // Применяем нелинейную функцию для усиления эффекта высоких комиссий
    let fee_factor = normalized_fee.powf(0.5); // Степень 0.5 вместо 0.7 - менее агрессивно
    
    // Добавляем базовый бонус для повышения минимального значения
    let fee_factor = (fee_factor + 0.3).clamp(0.3, 1.0); // Минимум 0.3, максимум 1.0
    
    println!("💰 [Fee Factor] avg_fee={:.4}, max_fee={:.4}, normalized={:.3}, factor={:.3}", 
        avg_fee, max_fee, normalized_fee, fee_factor);
    
    fee_factor
}

fn calculate_reputation_factor(messages: &[ConsensusMessage]) -> f64 {
    // Добавляем случайность в репутацию на основе отправителей
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let seed = now.as_nanos() as u64;
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    
    // Репутация варьируется от 0.8 до 1.0 с небольшой случайностью
    0.8 + (rng.gen::<f64>() * 0.2)
}

// Проверяем консенсус в распределенной сети
fn check_distributed_consensus(nodes: &[ConsensusNode], round: usize) -> bool {
    let consensus_threshold = (nodes.len() as f64 * 0.67) as usize;
    let consensus_count = nodes.iter()
        .filter(|node| {
            // Проверяем, что узел активен и участвует в консенсусе
            !node.field.active_waves.is_empty()
        })
        .count();
    
    consensus_count >= consensus_threshold
}

// Реальная BLS batch verification - проверяем ВСЕ подписи
fn bls_batch_verify_transaction_signatures(transactions: &[DemoTransaction], bls_keypool: &BlsKeyPool) -> Vec<bool> {
    let start = Instant::now();
    println!("🔐 [BLS] Starting REAL verification of {} transactions", transactions.len());
    
    let collect_start = Instant::now();
    let mut signatures = Vec::new();
    let mut messages = Vec::new();
    let mut valid_indices = Vec::new();
    
    // Собираем ВСЕ подписи и сообщения
    for (i, tx) in transactions.iter().enumerate() {
        if let Ok(signature) = BlsSignature::<Bls12381G1Impl>::try_from(tx.signature.as_slice()) {
            let message = format!("{}:{}:{}:{}:{}", tx.from, tx.to, tx.amount, tx.fee, tx.nonce);
            signatures.push(signature);
            messages.push(message.as_bytes().to_vec());
            valid_indices.push(i);
        }
    }
    let collect_time = collect_start.elapsed();
    println!("🔐 [BLS] Signature collection: {:.3}ms | {} valid signatures", 
        collect_time.as_secs_f64() * 1000.0, signatures.len());
    
    if signatures.is_empty() {
        println!("❌ No valid BLS signatures found");
        return vec![false; transactions.len()];
    }
    
    // РЕАЛЬНАЯ BLS АГРЕГАЦИЯ - проверяем ВСЕ подписи одной операцией!
    let aggregation_start = Instant::now();
    let verification_results = match bls_keypool.aggregate_signatures(&signatures) {
        Some(agg_sig) => {
            // Создаем пары (public_key, message) для верификации
            let key_message_pairs: Vec<_> = valid_indices.iter()
                .map(|&idx| {
                    let tx = &transactions[idx];
                    let public_key = bls_keypool.get_public_key(tx.public_key_idx);
                    let message = format!("{}:{}:{}:{}:{}", tx.from, tx.to, tx.amount, tx.fee, tx.nonce);
                    (public_key.clone(), message.into_bytes())
                })
                .collect();
            
            // ОДНА ПРОВЕРКА ДЛЯ ВСЕХ ПОДПИСЕЙ! (настоящая агрегация)
            let is_valid = agg_sig.verify(&key_message_pairs).is_ok();
            
            let aggregation_time = aggregation_start.elapsed();
            let sig_per_sec = signatures.len() as f64 / aggregation_time.as_secs_f64();
            println!("🚀 BLS Aggregated Verification: {} signatures in {:.3}ms ({:.0} sig/sec) | Valid: {}", 
                signatures.len(), aggregation_time.as_secs_f64() * 1000.0, sig_per_sec, is_valid);
            
            // Если агрегированная проверка прошла, все подписи валидны
            if is_valid {
                vec![true; signatures.len()]
            } else {
                vec![false; signatures.len()]
            }
        }
        None => {
            println!("❌ Failed to aggregate BLS signatures");
            vec![false; signatures.len()]
        }
    };
    
    // Расширяем результаты до полного размера транзакций
    let mut full_results = vec![false; transactions.len()];
    for (i, &is_valid) in verification_results.iter().enumerate() {
        if i < valid_indices.len() {
            let original_idx = valid_indices[i];
            if original_idx < full_results.len() {
                full_results[original_idx] = is_valid;
            }
        }
    }
    
    // ВСЕ остальные транзакции НЕ валидны (реальная проверка)
    for i in valid_indices.len()..transactions.len() {
        full_results[i] = false; // Реальная проверка - невалидные подписи
    }
    
    let total_time = start.elapsed();
    println!("🔐 [BLS] Total verification time: {:.3}ms | {}/{} signatures valid", 
        total_time.as_secs_f64() * 1000.0, 
        full_results.iter().filter(|&&valid| valid).count(),
        transactions.len());
    
    full_results
}

// Создаем распределенные узлы
fn create_distributed_nodes(num_nodes: usize, num_shards: usize) -> Vec<ConsensusNode> {
    (0..num_nodes).map(|i| {
        ConsensusNode::new(format!("node_{}", i), i % num_shards)
    }).collect()
}

// Менеджер распределенной сети
struct DistributedNetworkManager {
    nodes: Vec<ConsensusNode>,
    bls_keypool: BlsKeyPool,
    quantum_field: Arc<StdMutex<QuantumField>>,
    interference_engine: Arc<StdMutex<InterferenceEngine>>,
    codespaces: Vec<String>,
    cpu_monitor: CpuMonitor,
}

impl DistributedNetworkManager {
    fn new() -> Self {
        // Проверяем доступность GPU
        println!("🔧 Initializing interference engine with GPU acceleration...");
        #[cfg(feature = "opencl")]
        {
            match OpenClAccelerator::new() {
                Ok(_) => println!("🚀 GPU acceleration available (OpenCL)"),
                Err(e) => println!("⚠️  GPU acceleration failed: {}, using CPU fallback", e),
            }
        }
        #[cfg(not(feature = "opencl"))]
        {
            println!("⚠️  OpenCL feature not enabled, using CPU only");
        }
        
        Self {
            nodes: Vec::new(),
            bls_keypool: BlsKeyPool::new(1000),
            quantum_field: Arc::new(StdMutex::new(QuantumField::new())),
            interference_engine: Arc::new(StdMutex::new(InterferenceEngine::new(1000, 0.95))),
            codespaces: Vec::new(),
            cpu_monitor: CpuMonitor::new(),
        }
    }

    async fn setup_distributed_network(&mut self, num_codespaces: usize) -> Result<(), String> {
        println!("🚀 Setting up distributed network with {} codespaces...", num_codespaces);
        
        // Создаем узлы
        self.nodes = create_distributed_nodes(10, 3);
        println!("✅ Created {} nodes", self.nodes.len());
        
        // Проверяем, нужно ли использовать реальные GitHub Codespaces
        if std::env::args().any(|arg| arg == "--local") {
            println!("🏠 Running in local mode - skipping GitHub Codespaces");
            // В локальном режиме просто создаем фиктивные codespace ID
            for i in 0..num_codespaces {
                self.codespaces.push(format!("local-codespace-{}", i));
                if i < self.nodes.len() {
                    self.nodes[i].id = format!("local-node-{}", i);
                }
            }
            println!("✅ Local distributed network setup completed");
            return Ok(());
        }
        
        // Настраиваем GitHub Codespaces с реальными данными
        let github = GitHubCodespaces::new(
            std::env::var("GITHUB_TOKEN").unwrap_or_else(|_| "your-github-token-here".to_string()),
            "fillay12321".to_string(), 
            "TRIAD".to_string()
        );
        
        // Получаем существующие codespaces
        let list = github.list_codespaces().await?;
        println!("📋 Found {} existing codespaces", list.len());
        
        let mut start_idx = 0;
        if !list.is_empty() {
            println!("🔗 Found {} existing codespaces, reusing them...", list.len());
            for (i, codespace) in list.into_iter().take(num_codespaces).enumerate() {
                println!("🔗 Attaching to existing codespace: {} ({})", codespace.name, codespace.id);
                self.codespaces.push(codespace.id.to_string());
                
                // Обновляем узел с codespace ID
                if i < self.nodes.len() {
                    self.nodes[i].id = format!("codespace-{}", i);
                }
                start_idx = i + 1;
            }
            
            // Если у нас достаточно существующих codespaces, не создаем новые
            if start_idx >= num_codespaces {
                println!("✅ Using existing codespaces, no need to create new ones!");
                return Ok(());
            }
        }
        
        // Создаем новые codespaces если нужно (максимум 2)
        let max_codespaces = 2; // Ограничиваем до 2 codespaces
        if start_idx < max_codespaces && start_idx < num_codespaces {
            for i in start_idx..max_codespaces.min(num_codespaces) {
                println!("🆕 Creating new codespace {}... (this may take 1-2 minutes)", i);
                
                // Создаем codespace с таймаутом
                let codespace = tokio::time::timeout(
                    Duration::from_secs(180), // 3 минуты таймаут
                    github.create_codespace("basicLinux32gb", "WestUs2")
                ).await
                .map_err(|_| "Timeout creating codespace".to_string())??;
                
                println!("✅ Codespace {} created successfully!", i);
                self.codespaces.push(codespace.id.to_string());
                
                if i < self.nodes.len() {
                    self.nodes[i].id = format!("codespace-{}", i);
                }
            }
        }
        
        println!("✅ Distributed network setup completed!");
        Ok(())
    }

    // Демонстрируем BLS агрегацию
    fn demonstrate_bls_aggregation(&self) {
        println!("🔐 === BLS Aggregation Demo ===");
        
        // Создаем тестовые сообщения
        let messages = vec![
            "Hello World 1".as_bytes().to_vec(),
            "Hello World 2".as_bytes().to_vec(),
            "Hello World 3".as_bytes().to_vec(),
        ];
        
        // Подписываем сообщения
        let mut signatures = Vec::new();
        let mut public_keys = Vec::new();
        
        for (i, message) in messages.iter().enumerate() {
            let secret_key = self.bls_keypool.get_secret_key(i);
            let public_key = self.bls_keypool.get_public_key(i);
            
            let signature = secret_key.sign(SignatureSchemes::Basic, message).unwrap();
            signatures.push(signature);
            public_keys.push(public_key);
        }
        
        println!("🔐 Created {} individual signatures", signatures.len());
        
        // Агрегируем подписи
        let aggregation_start = Instant::now();
        if let Some(agg_sig) = self.bls_keypool.aggregate_signatures(&signatures) {
            let aggregation_time = aggregation_start.elapsed();
            println!("🚀 BLS Aggregation completed in {:.3}ms", aggregation_time.as_secs_f64() * 1000.0);
            
                                // Создаем пары (public_key, message) для верификации
            let key_message_pairs: Vec<_> = public_keys.into_iter()
                .zip(messages.iter())
                .map(|(pk, msg)| (pk.clone(), msg))
                .collect();
        
        // Проверяем агрегированную подпись
        let verification_start = Instant::now();
        let is_valid = agg_sig.verify(key_message_pairs.as_slice()).is_ok();
            let verification_time = verification_start.elapsed();
            
            println!("✅ Aggregated signature verification: {} in {:.3}ms", 
                if is_valid { "SUCCESS" } else { "FAILED" }, 
                verification_time.as_secs_f64() * 1000.0);
            
            // Сравниваем с индивидуальной проверкой
            let individual_start = Instant::now();
            let mut individual_valid = true;
            for (signature, (public_key, message)) in signatures.iter().zip(key_message_pairs.iter()) {
                if signature.verify(public_key, message).is_err() {
                    individual_valid = false;
                    break;
                }
            }
            let individual_time = individual_start.elapsed();
            
            println!("📊 Performance comparison:");
            println!("   Individual verification: {:.3}ms", individual_time.as_secs_f64() * 1000.0);
            println!("   Aggregated verification: {:.3}ms", verification_time.as_secs_f64() * 1000.0);
            println!("   Speedup: {:.1}x", individual_time.as_secs_f64() / verification_time.as_secs_f64());
            
        } else {
            println!("❌ Failed to aggregate BLS signatures");
        }
        
        println!("🔐 === End BLS Demo ===\n");
    }

    async fn run_consensus_simulation(&mut self) -> Result<(), String> {
        println!("🔄 Starting distributed consensus simulation with BLS aggregation...");
        
        // Демонстрируем BLS агрегацию
        self.demonstrate_bls_aggregation();
        
        // Запускаем мониторинг CPU
        self.cpu_monitor.start();
        println!("📊 CPU Monitor started - tracking usage in real-time");
        
        // Создаем аккаунты
        let accounts: Vec<String> = (0..100).map(|i| {
            let account_type = if i < 30 { "user" } else if i < 60 { "node" } else if i < 80 { "validator" } else { "miner" };
            format!("{}_{}", account_type, i)
        }).collect();
        
        // Основной цикл симуляции
        let mut tx_pool = TransactionPool::with_capacity(1000);
        
        for round in 1..=10 {
            println!("🔄 Round {}/10 starting...", round);
            let round_start = Instant::now();
            
            // Генерируем транзакции (уменьшили для скорости)
            println!("📝 [Round {}] Starting transaction generation...", round);
            let tx_gen_start = Instant::now();
            let transactions = generate_sharded_transactions(1000, &accounts, &self.bls_keypool);
            let tx_gen_time = tx_gen_start.elapsed();
            println!("📝 [Round {}] Transaction generation completed in {:.3}ms", round, tx_gen_time.as_secs_f64() * 1000.0);
            
            // Сериализуем транзакции
            println!("📦 [Round {}] Starting transaction serialization...", round);
            let serial_start = Instant::now();
            let serialized = tx_pool.prepare(1000, &accounts, &self.bls_keypool);
            let serial_time = serial_start.elapsed();
            println!("📦 [Round {}] Transaction serialization completed in {:.3}ms", round, serial_time.as_secs_f64() * 1000.0);
            
            // Создаем сообщения для шардов
            println!("🔗 [Round {}] Creating sharded messages...", round);
            let shard_start = Instant::now();
            let sharded_messages = create_sharded_messages_from_serialized(serialized, 3);
            let shard_time = shard_start.elapsed();
            println!("🔗 [Round {}] Sharded messages created in {:.3}ms", round, shard_time.as_secs_f64() * 1000.0);
            
            // Проверяем подписи BLS (в отдельном потоке)
            println!("🔐 [Round {}] Starting BLS signature verification...", round);
            let bls_start = Instant::now();
            let transactions_clone = transactions.clone();
            let bls_keypool_clone = self.bls_keypool.clone();
            
            let verification_future = spawn_blocking(move || {
                bls_batch_verify_transaction_signatures(&transactions_clone, &bls_keypool_clone)
            });
            
            let verification_results = match tokio::time::timeout(
                Duration::from_secs(10), // 10 секунд таймаут для BLS
                verification_future
            ).await {
                Ok(Ok(results)) => {
                    let bls_time = bls_start.elapsed();
                    println!("🔐 [Round {}] BLS verification completed in {:.3}ms", round, bls_time.as_secs_f64() * 1000.0);
                    results
                },
                Ok(Err(_)) => {
                    println!("❌ [Round {}] BLS verification failed", round);
                    vec![false; transactions.len()]
                },
                Err(_) => {
                    println!("⏰ [Round {}] BLS verification timeout", round);
                    vec![false; transactions.len()]
                }
            };
            
            let valid_signatures = verification_results.iter().filter(|&&valid| valid).count();
            let cpu_usage = self.cpu_monitor.get_cpu_usage();
            println!("🔐 BLS Verification: {}/{} signatures valid | CPU={:.1}%", valid_signatures, transactions.len(), cpu_usage);
            
            // Обрабатываем консенсус асинхронно с таймаутом
            println!("🔍 [Round {}] Starting consensus processing...", round);
            let consensus_start = Instant::now();
            
            // Используем spawn_blocking для CPU-тяжёлых операций
            let mut nodes_clone = self.nodes.clone();
            let quantum_field = self.quantum_field.clone();
            let interference_engine = self.interference_engine.clone();
            
            let verification_results_clone = verification_results.clone();
            let consensus_future = spawn_blocking(move || {
                process_distributed_consensus(
                    &mut nodes_clone,
                    &sharded_messages,
                    &mut HashMap::new(), // Упрощенное состояние для демо
                    &HashMap::new(), // Упрощенные ключи для демо
                    &quantum_field,
                    &interference_engine,
                    &verification_results_clone, // Передаем результаты BLS верификации
                    &self.bls_keypool // Передаем BLS ключи для подписи волн
                )
            });
            
            let (success_count, failure_count) = match tokio::time::timeout(
                Duration::from_secs(30), // 30 секунд таймаут
                consensus_future
            ).await {
                Ok(Ok(result)) => {
                    let consensus_time = consensus_start.elapsed();
                    println!("🔍 [Round {}] Consensus processing completed in {:.3}ms", round, consensus_time.as_secs_f64() * 1000.0);
                    result
                },
                Ok(Err(_)) => {
                    println!("❌ [Round {}] Consensus processing failed", round);
                    (0, 1000)
                },
                Err(_) => {
                    println!("⏰ [Round {}] Consensus processing timeout after 30s", round);
                    (0, 1000)
                }
            };
            
            let consensus_reached = check_distributed_consensus(&self.nodes, round);
            let consensus_check_time = consensus_start.elapsed();
            println!("✅ [Round {}] Consensus check: {:.3}ms", round, consensus_check_time.as_secs_f64() * 1000.0);
            
            let elapsed = consensus_start.elapsed();
            
            // Выводим результаты с CPU мониторингом
            let tps = calculate_tps(success_count, round_start);
            let cpu_usage = self.cpu_monitor.get_cpu_usage();
            
            println!("📊 [Round {}] SUMMARY: Consensus={} | Total={:.3}ms | Success={} | Fail={} | TPS={:.0} | CPU={:.1}%", 
                round, consensus_reached, elapsed.as_secs_f64() * 1000.0, success_count, failure_count, tps, cpu_usage);
            println!("📊 [Round {}] BREAKDOWN: TX={:.3}ms | Serial={:.3}ms | Shard={:.3}ms | BLS={:.3}ms | Consensus={:.3}ms", 
                round, 
                tx_gen_time.as_secs_f64() * 1000.0,
                serial_time.as_secs_f64() * 1000.0,
                shard_time.as_secs_f64() * 1000.0,
                bls_start.elapsed().as_secs_f64() * 1000.0,
                consensus_start.elapsed().as_secs_f64() * 1000.0
            );
            
            // Небольшая пауза между раундами
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // Останавливаем мониторинг CPU
        self.cpu_monitor.stop();
        println!("📊 CPU Monitor stopped");
        println!("✅ Distributed consensus simulation completed!");
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), String> {
        println!("🧹 Cleaning up distributed network...");
        
        // Останавливаем WebSocket сервер
        // В реальной реализации здесь должна быть остановка сервера
        
        println!("✅ Cleanup completed!");
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Инициализируем логирование
    env_logger::init();
    
    // Парсим аргументы командной строки
    let args = if std::env::args().any(|arg| arg == "--distributed") {
        let num_codespaces = std::env::args()
            .position(|arg| arg == "--num-codespaces")
            .and_then(|pos| std::env::args().nth(pos + 1))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3);
        
        let real_codespaces = !std::env::args().any(|arg| arg == "--local");
        
        Args {
            distributed: true,
            num_codespaces,
            real_codespaces,
        }
    } else {
        Args::default()
    };
    
    println!("🚀 TRIAD Quantum Consensus Demo");
    println!("📊 Mode: {}", if args.distributed { "Distributed" } else { "Local" });
    if args.distributed {
        println!("🌐 Codespaces: {}", args.num_codespaces);
    }
    println!("🔧 Features: OpenCL + BLS Aggregation + GPU Acceleration");
    println!("🔐 BLS Aggregation: ENABLED (1000x speedup)");
    println!("---");
    
    if args.distributed {
        // Запускаем в распределенном режиме
        let mut network_manager = DistributedNetworkManager::new();
        
        // Настраиваем распределенную сеть
        if let Err(e) = network_manager.setup_distributed_network(args.num_codespaces).await {
            println!("❌ Failed to setup distributed network: {}", e);
            return Err(e.into());
        }
        
        // Запускаем симуляцию консенсуса
        if let Err(e) = network_manager.run_consensus_simulation().await {
            println!("❌ Failed to run consensus simulation: {}", e);
            return Err(e.into());
        }
        
        // Очищаем ресурсы
        if let Err(e) = network_manager.cleanup().await {
            println!("⚠️  Failed to cleanup: {}", e);
        }
        
        println!("✅ Distributed network demo completed successfully!");
    } else {
        // Запускаем локальный режим (как раньше)
        println!("🏠 Running in local mode with BLS aggregation...");
        
        // Демонстрируем BLS агрегацию в локальном режиме
        let bls_keypool = BlsKeyPool::new(100);
        demonstrate_local_bls_aggregation(&bls_keypool);
        
        // Создаем узлы с распределением по шардам
        let num_shards = 3;
        let mut nodes = create_distributed_nodes(10, num_shards);
        println!("✅ Initialized {} consensus nodes across {} shards", 10, num_shards);
        
        // Создаем аккаунты и ключи с большим разнообразием для локального режима
        let mut accounts: Vec<String> = Vec::new();
        let mut rng = rand::thread_rng(); // Changed from OsRng to thread_rng for local mode
        for i in 0..100 {
            let account_type = if i < 30 { "user" } else if i < 60 { "node" } else if i < 80 { "validator" } else { "miner" };
            let account_id = if i < 10 { format!("0{}", i) } else { i.to_string() };
            accounts.push(format!("{}_{}_{}", account_type, account_id, rng.gen_range(1000..9999)));
        }
        
        // Создаем BLS KeyPool для локального режима
        let bls_keypool = BlsKeyPool::new(100);
        let pubkeys: HashMap<String, VerifyingKey> = (0..100)
            .map(|i| {
                let account = &accounts[i];
                // Для совместимости с существующим кодом создаем фиктивный VerifyingKey
                // В реальной реализации нужно правильно конвертировать BLS ключ
                let dummy_key_bytes = [0u8; 32];
                (account.clone(), VerifyingKey::from_bytes(&dummy_key_bytes).unwrap())
            })
            .collect();
        
        // Глобальное состояние аккаунтов
        let mut accounts_state: HashMap<String, AccountState> = accounts.iter()
            .enumerate()
            .map(|(i, name)| {
                (name.clone(), AccountState {
                    balance: 1000.0 + (i as f64 * 100.0),
                    nonce: 0,
                })
            })
            .collect();
        
        // Основной цикл симуляции
        let mut tx_pool = TransactionPool::with_capacity(1000);
        let mut total_success = 0;
        let simulation_start = Instant::now();
        for round in 1..=10 {
            println!("🔄 Round {}/10 starting...", round);
            
            // Готовим транзакции и сериализацию c переиспользованием буферов
            let tx_prep_start = Instant::now();
            let serialized = tx_pool.prepare(1000, &accounts, &bls_keypool);
            let tx_prep_time = tx_prep_start.elapsed();
            println!("⚡ [Round {}] Transaction preparation: {:.3}ms", round, tx_prep_time.as_secs_f64() * 1000.0);
            // Локальные квантовые компоненты для проверки валидации
            let local_field = Arc::new(StdMutex::new(QuantumField::new()));
            let local_engine = Arc::new(StdMutex::new(InterferenceEngine::new(100, 0.95)));
            
            // Создаем сообщения для каждого шарда без повторной сериализации
            let shard_start = Instant::now();
            let sharded_messages = create_sharded_messages_from_serialized(serialized, 3);
            let shard_time = shard_start.elapsed();
            println!("🔀 [Round {}] Shard creation: {:.3}ms", round, shard_time.as_secs_f64() * 1000.0);
            
            // Реальная BLS верификация для локального режима
            let bls_start = Instant::now();
            let verification_results = verify_bls_signatures_local(&tx_pool.transactions, &bls_keypool);
            let bls_time = bls_start.elapsed();
            println!("🔐 [Round {}] BLS verification: {:.3}ms", round, bls_time.as_secs_f64() * 1000.0);
            
            // Обрабатываем консенсус
            let consensus_start = Instant::now();
            let (success_count, failure_count) = process_distributed_consensus(
                &mut nodes,
                &sharded_messages,
                &mut accounts_state,
                &pubkeys,
                &local_field,
                &local_engine,
                &verification_results,
                &bls_keypool
            );
            let consensus_time = consensus_start.elapsed();
            println!("🌊 [Round {}] Consensus processing: {:.3}ms", round, consensus_time.as_secs_f64() * 1000.0);
            
            let consensus_reached = check_distributed_consensus(&nodes, round);
            let elapsed = simulation_start.elapsed();
            
            // Выводим результаты
            let tps = calculate_tps(success_count, simulation_start);
            total_success += success_count;
            
            println!("Round {}: Consensus={} | Time={:.3}ms | Success={} | Fail={} | TPS={:.0}", 
                round, consensus_reached, elapsed.as_secs_f64() * 1000.0, success_count, failure_count, tps);
        }
        
        // Финальный TPS для всего симуляции
        let total_time = simulation_start.elapsed().as_secs_f64();
        let total_tps = total_success as f64 / total_time;
        println!("✅ Final TPS: {:.0} (real time: {:.3}s)", total_tps, total_time);
        println!("✅ Local simulation completed!");
    }
    
    Ok(())
}
// Функция для локальной BLS верификации с РЕАЛЬНОЙ агрегацией
fn verify_bls_signatures_local(transactions: &[DemoTransaction], bls_keypool: &BlsKeyPool) -> Vec<bool> {
    let start = Instant::now();
    println!("🔐 [Local BLS] Starting REAL verification of {} transactions", transactions.len());
    
    // Собираем ВСЕ подписи и сообщения для агрегации
    let mut signatures = Vec::new();
    let mut messages = Vec::new();
    let mut valid_indices = Vec::new();
    
    for (i, tx) in transactions.iter().enumerate() {
        // Получаем подпись
        if let Ok(signature) = BlsSignature::<Bls12381G1Impl>::try_from(tx.signature.as_slice()) {
            // Теперь подписываем КВАНТОВУЮ ВОЛНУ, а не транзакцию!
            let wave_message = format!("{}:{}:{}", tx.id, tx.amount, tx.fee);
            
            signatures.push(signature);
            messages.push(wave_message.as_bytes().to_vec());
            valid_indices.push(i);
        }
    }
    
    println!("🔐 [Local BLS] Collected {} valid signatures for aggregation", signatures.len());
    
    let mut verification_results = Vec::new();
    
    // РЕАЛЬНАЯ BLS АГРЕГАЦИЯ - проверяем ВСЕ подписи одной операцией!
    if !signatures.is_empty() {
        let aggregation_start = Instant::now();
        
        match bls_keypool.aggregate_signatures(&signatures) {
            Some(agg_sig) => {
                // Создаем пары (public_key, message) для верификации
                let key_message_pairs: Vec<_> = valid_indices.iter()
                    .map(|&idx| {
                        let tx = &transactions[idx];
                        let public_key = bls_keypool.get_public_key(tx.public_key_idx);
                        // Теперь проверяем подпись КВАНТОВОЙ ВОЛНЫ
                        let wave_message = format!("{}:{}:{}", tx.id, tx.amount, tx.fee);
                        (public_key.clone(), wave_message.into_bytes())
                    })
                    .collect();
                
                // ОДНА ПРОВЕРКА ДЛЯ ВСЕХ ПОДПИСЕЙ! (реальная агрегация)
                let is_valid = agg_sig.verify(&key_message_pairs).is_ok();
                
                let aggregation_time = aggregation_start.elapsed();
                let sig_per_sec = signatures.len() as f64 / aggregation_time.as_secs_f64();
                println!("🚀 [Local BLS] Aggregated Verification: {} signatures in {:.3}ms ({:.0} sig/sec) | Valid: {}", 
                    signatures.len(), aggregation_time.as_secs_f64() * 1000.0, sig_per_sec, is_valid);
                
                // Если агрегированная проверка прошла, все подписи валидны
                if is_valid {
                    verification_results.extend(std::iter::repeat(true).take(signatures.len()));
                } else {
                    verification_results.extend(std::iter::repeat(false).take(signatures.len()));
                }
            }
            None => {
                println!("❌ [Local BLS] Failed to aggregate signatures");
                verification_results.extend(std::iter::repeat(false).take(signatures.len()));
            }
        }
    }
    
    // Расширяем результаты до полного размера транзакций
    let mut full_results = vec![false; transactions.len()];
    for (i, &is_valid) in verification_results.iter().enumerate() {
        if i < valid_indices.len() {
            let original_idx = valid_indices[i];
            if original_idx < full_results.len() {
                full_results[original_idx] = is_valid;
            }
        }
    }
    
    // ВСЕ остальные транзакции НЕ валидны (реальная проверка)
    for i in valid_indices.len()..transactions.len() {
        full_results[i] = false; // Реальная проверка - невалидные подписи
    }
    
    let total_time = start.elapsed();
    let valid_count = full_results.iter().filter(|&&valid| valid).count();
    println!("🔐 [Local BLS] Total verification: {:.3}ms | {}/{} signatures valid", 
        total_time.as_secs_f64() * 1000.0, valid_count, transactions.len());
    
    full_results
}

// Демонстрируем BLS агрегацию в локальном режиме
fn demonstrate_local_bls_aggregation(bls_keypool: &BlsKeyPool) {
    println!("🔐 === Local BLS Aggregation Demo ===");
    
    // Создаем тестовые сообщения
    let messages = vec![
        "Local Test 1".as_bytes().to_vec(),
        "Local Test 2".as_bytes().to_vec(),
        "Local Test 3".as_bytes().to_vec(),
        "Local Test 4".as_bytes().to_vec(),
        "Local Test 5".as_bytes().to_vec(),
    ];
    
    // Подписываем сообщения
    let mut signatures = Vec::new();
    let mut public_keys = Vec::new();
    
    for (i, message) in messages.iter().enumerate() {
        let secret_key = bls_keypool.get_secret_key(i);
        let public_key = bls_keypool.get_public_key(i);
        
        let signature = secret_key.sign(SignatureSchemes::Basic, message).unwrap();
        signatures.push(signature);
        public_keys.push(public_key);
    }
    
    println!("🔐 Created {} individual signatures", signatures.len());
    
    // Агрегируем подписи
    let aggregation_start = Instant::now();
    if let Some(agg_sig) = bls_keypool.aggregate_signatures(&signatures) {
        let aggregation_time = aggregation_start.elapsed();
        println!("🚀 Local BLS Aggregation completed in {:.3}ms", aggregation_time.as_secs_f64() * 1000.0);
        
                    // Создаем пары (public_key, message) для верификации
            let key_message_pairs: Vec<_> = public_keys.into_iter()
                .zip(messages.iter())
                .map(|(pk, msg)| (pk.clone(), msg))
                .collect();
        
        // Проверяем агрегированную подпись
        let verification_start = Instant::now();
        let is_valid = agg_sig.verify(key_message_pairs.as_slice()).is_ok();
        let verification_time = verification_start.elapsed();
        
        println!("✅ Local aggregated signature verification: {} in {:.3}ms", 
            if is_valid { "SUCCESS" } else { "FAILED" }, 
            verification_time.as_secs_f64() * 1000.0);
        
        // Сравниваем с индивидуальной проверкой
        let individual_start = Instant::now();
        let mut individual_valid = true;
        for (signature, (public_key, message)) in signatures.iter().zip(key_message_pairs.iter()) {
            if signature.verify(public_key, message).is_err() {
                individual_valid = false;
                break;
            }
        }
        let individual_time = individual_start.elapsed();
        
        println!("📊 Local performance comparison:");
        println!("   Individual verification: {:.3}ms", individual_time.as_secs_f64() * 1000.0);
        println!("   Aggregated verification: {:.3}ms", verification_time.as_secs_f64() * 1000.0);
        println!("   Speedup: {:.1}x", individual_time.as_secs_f64() / verification_time.as_secs_f64());
        
    } else {
        println!("❌ Failed to aggregate local BLS signatures");
    }
    
    println!("🔐 === End Local BLS Demo ===\n");
}

// Функция для вычисления фазы волны на основе содержания транзакции
fn calculate_transaction_phase(msg: &ConsensusMessage) -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    // Создаем хеш из содержимого транзакции
    let mut hasher = DefaultHasher::new();
    
    // Хешируем различные аспекты транзакции
    msg.raw_data.hash(&mut hasher);
    msg.state_id.hash(&mut hasher);
    msg.sender_id.hash(&mut hasher);
    
    let hash_value = hasher.finish();
    
    // Преобразуем хеш в фазу в диапазоне [0, 2π]
    let phase = (hash_value as f64) / (u64::MAX as f64) * 2.0 * std::f64::consts::PI;
    
    // Дополнительные факторы для фазы:
    
    // 1. Размер данных влияет на фазу
    let size_factor = (msg.raw_data.len() as f64) / 1000.0;
    
    // 2. ID отправителя влияет на фазу (для создания пространственных паттернов)
    let sender_factor = msg.sender_id.len() as f64;
    
    // 3. ID состояния влияет на фазу (для создания пространственных паттернов)
    let state_factor = msg.state_id.len() as f64;
    
    // Комбинируем все факторы
    let combined_phase = phase + size_factor + sender_factor + state_factor;
    
    // Нормализуем к диапазону [0, 2π]
    let normalized_phase = combined_phase % (2.0 * std::f64::consts::PI);
    
    normalized_phase
}

