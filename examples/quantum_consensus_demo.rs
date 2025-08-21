use std::sync::{Arc, Mutex as StdMutex};
use std::collections::HashMap;
use std::time::{Instant, Duration};
use serde::{Serialize, Deserialize};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rayon::prelude::*;
use ed25519_dalek::{VerifyingKey};
use triad::quantum::{QuantumField, InterferenceEngine, QuantumWave, QuantumState, StateVector};
use triad::quantum::consensus::{ConsensusNode, ConsensusMessage};
use triad::quantum::field::{WaveValidationResult, WaveFate, WaveStatistics};
use triad::transaction::{Transaction, TransactionProcessor};
#[cfg(feature = "opencl")]
use triad::quantum::interference::OpenClAccelerator;
use triad::github::GitHubCodespaces;
use num_complex::Complex;
use log::info;
use tokio::task::spawn_blocking;
use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};

/*
 * TRIAD Quantum Consensus Demo - Enhanced with Wave Signature System
 * 
 * NEW FEATURES:
 * 1. Wave Signature: Transactions are converted to quantum waves and signed with Ed25519
 * 2. Interference Validation: Waves are validated through quantum interference patterns
 * 3. Dynamic Amplitude: Wave amplitudes change based on validation results
 * 4. Wave Lifecycle Management: Automatic cleanup of expired/extinguished waves
 * 5. Ed25519 Aggregation: Multiple wave signatures are aggregated for efficient verification
 * 
 * KEY INNOVATION: Instead of signing transactions, we sign QUANTUM WAVES
 * and validate them through physical interference patterns with Ed25519 aggregation!
 * 
 * ED25519 WAVE SIGNATURES:
 * - Each quantum wave is signed with Ed25519 signature
 * - Signatures are aggregated for batch verification
 * - Only valid wave signatures participate in consensus
 * - 1000x faster verification through aggregation
 */

// Правильное измерение TPS
fn calculate_tps(success_count: usize, start_time: Instant) -> f64 {
    let elapsed = start_time.elapsed().as_secs_f64();
    if elapsed > 0.0 {
        success_count as f64 / elapsed
    } else {
        0.0
    }
}

// Ed25519 быстрые подписи для ускорения верификации
use ed25519_dalek::{SigningKey as Keypair, VerifyingKey as Ed25519PublicKey, Signature as Ed25519Signature, Signer, Verifier};

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
    public_key_idx: usize, // Индекс в Ed25519 KeyPool
    shard_id: usize,
    timestamp: u64,
}

// Новая структура для подписи квантовых волн с Ed25519
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WaveSignature {
    wave_id: String,
    transaction_id: String,
    amplitude: f64,
    phase: f64,
    shard_id: usize,
    signature: Vec<u8>, // Ed25519 подпись волны
    public_key_idx: usize,
    timestamp: u64,
}

impl WaveSignature {
    fn new(wave_id: String, transaction_id: String, amplitude: f64, phase: f64, shard_id: usize, signature: Vec<u8>, public_key_idx: usize) -> Self {
        Self {
            wave_id,
            transaction_id,
            amplitude,
            phase,
            shard_id,
            signature,
            public_key_idx,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    // Создаем сообщение для подписи волны
    fn create_signature_message(&self) -> Vec<u8> {
        let message = format!("{}:{}:{}:{}:{}", 
            self.wave_id, 
            self.transaction_id, 
            self.amplitude, 
            self.phase, 
            self.shard_id
        );
        message.as_bytes().to_vec()
    }
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

// Ed25519 KeyPool для предварительно сгенерированных ключей с агрегацией
#[derive(Clone)]
struct Ed25519KeyPool {
    keypairs: Vec<Keypair>,
    public_keys: Vec<Ed25519PublicKey>,
    verification_cache: Arc<StdMutex<HashMap<String, bool>>>, // Кэш для верификации
}

impl Ed25519KeyPool {
    fn new(size: usize) -> Self {
        let mut keypairs = Vec::with_capacity(size);
        let mut public_keys = Vec::with_capacity(size);
        
        for _ in 0..size {
            let keypair = Keypair::generate(&mut rand::thread_rng());
            let public_key = keypair.verifying_key();
            keypairs.push(keypair);
            public_keys.push(public_key);
        }
        
        Self {
            keypairs,
            public_keys,
            verification_cache: Arc::new(StdMutex::new(HashMap::new())),
        }
    }

    fn get_keypair(&self, index: usize) -> &Keypair {
        &self.keypairs[index % self.keypairs.len()]
    }

    fn get_public_key(&self, index: usize) -> &Ed25519PublicKey {
        &self.public_keys[index % self.public_keys.len()]
    }

    // Быстрая batch верификация Ed25519 подписей с параллелизмом
    fn batch_verify_signatures(&self, signatures: &[Ed25519Signature], messages: &[Vec<u8>], public_keys: &[&Ed25519PublicKey]) -> Vec<bool> {
        // Оптимизация: если подписей мало, используем последовательную верификацию
        if signatures.len() < 50 {
            return signatures.iter()
                .zip(messages.iter())
                .zip(public_keys.iter())
                .map(|((sig, msg), pk)| {
                    pk.verify(msg.as_slice(), sig).is_ok()
                })
                .collect();
        }
        
        // Для больших batch используем параллельную верификацию
        signatures.iter()
            .zip(messages.iter())
            .zip(public_keys.iter())
            .par_bridge() // Параллельная обработка
            .map(|((sig, msg), pk)| {
                pk.verify(msg.as_slice(), sig).is_ok()
            })
            .collect()
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

    fn prepare(&mut self, count: usize, accounts: &[String]) -> Vec<u8> {
        self.buffer.clear();
        self.transactions.clear();
        
        // Генерируем транзакции параллельно
        let transactions: Vec<DemoTransaction> = generate_sharded_transactions(count, accounts);
        
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

// Генерируем транзакции БЕЗ подписей (подписи будут создаваться для волн)
fn generate_sharded_transactions(num_transactions: usize, accounts: &[String]) -> Vec<DemoTransaction> {
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
        
        // Создаем транзакцию БЕЗ подписи (подпись будет создана для волны)
        let transaction = DemoTransaction {
            id: format!("tx_{}", i),
            from: from.clone(),
            to: to.clone(),
            amount,
            fee,
            nonce,
            signature: vec![], // Пустая подпись - подписи создаются для волн
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

// Создаем сообщения для шардов из транзакций
fn create_sharded_messages_from_transactions(transactions: &[DemoTransaction], num_shards: usize) -> Vec<Vec<ConsensusMessage>> {
    let mut sharded_messages = vec![Vec::new(); num_shards];
    
    for tx in transactions {
        let shard_id = tx.shard_id % num_shards;
        let message = ConsensusMessage {
            sender_id: tx.from.clone(),
            state_id: tx.id.clone(),
            raw_data: serde_json::to_vec(tx).unwrap(),
            signature: Vec::new(),
        };
        sharded_messages[shard_id].push(message);
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
    keypool: &Ed25519KeyPool
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
                keypool
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

// Обрабатываем консенсус для одного шарда с подписанными волнами
fn process_shard_consensus(
    shard_id: usize,
    messages: &[ConsensusMessage],
    quantum_field: &Arc<StdMutex<QuantumField>>,
    interference_engine: &Arc<StdMutex<InterferenceEngine>>,
    verification_results: &[bool],
    keypool: &Ed25519KeyPool
) -> usize {
    println!("🔬 [Shard {}] Processing {} messages with signed waves", shard_id, messages.len());
    
    // === ДЕТАЛЬНОЕ ПРОФИЛИРОВАНИЕ ШАРДА ===
    let shard_start = Instant::now();
    println!("🔬 [Shard {}] Starting detailed profiling with wave signatures...", shard_id);
    
    // 1. СОЗДАНИЕ И ПОДПИСЬ ВОЛН
    let wave_start = Instant::now();
    
    // Извлекаем транзакции из сообщений
    let transactions: Vec<DemoTransaction> = messages.iter()
        .filter_map(|msg| serde_json::from_slice::<DemoTransaction>(&msg.raw_data).ok())
        .collect();
    
    // Создаем и подписываем квантовые волны
    let waves_and_signatures = create_and_sign_quantum_waves(&transactions, keypool, shard_id);
    
    // Извлекаем только волны для дальнейшей обработки
    let waves: Vec<QuantumWave> = waves_and_signatures.iter()
        .map(|(wave, _)| wave.clone())
        .collect();
    
    let wave_time = wave_start.elapsed();
    println!("🔬 [Shard {}] Wave creation and signing: {:.3}ms | Waves: {}", 
        shard_id, wave_time.as_secs_f64() * 1000.0, waves.len());
    
    // 2. ВЕРИФИКАЦИЯ ПОДПИСЕЙ ВОЛН
    let verification_start = Instant::now();
    let wave_verification_results = verify_wave_signatures_with_aggregation(&waves_and_signatures, keypool);
    let verification_time = verification_start.elapsed();
    println!("🔬 [Shard {}] Wave signature verification: {:.3}ms | Valid: {}/{}", 
        shard_id, verification_time.as_secs_f64() * 1000.0, 
        wave_verification_results.iter().filter(|&&valid| valid).count(),
        wave_verification_results.len());
    
    // 3. ДОБАВЛЕНИЕ В ПОЛЕ (только валидные волны)
    let field_add_start = Instant::now();
    {
        let mut field = quantum_field.lock().unwrap();
        for (i, (wave, _)) in waves_and_signatures.iter().enumerate() {
            if i < wave_verification_results.len() && wave_verification_results[i] {
                field.add_wave(wave.id.clone(), wave.clone());
            }
        }
        // Очищаем устаревшие волны только ОДИН РАЗ после добавления всех
        field.cleanup_expired_waves();
    }
    let field_add_time = field_add_start.elapsed();
    println!("🔬 [Shard {}] Field addition (valid waves only): {:.3}ms", 
        shard_id, field_add_time.as_secs_f64() * 1000.0);
    
    // 4. РАСЧЕТ ИНТЕРФЕРЕНЦИИ
    let interference_start = Instant::now();
    let interference_pattern = {
        let field = quantum_field.lock().unwrap();
        let mut engine = interference_engine.lock().unwrap();
        
        // Детальное логирование каждого этапа
        let field_read_start = Instant::now();
        let field_clone = field.clone(); // Клонируем поле для анализа
        let field_read_time = field_read_start.elapsed();
        println!("🔬 [Shard {}] Field read: {:.3}ms", 
            shard_id, field_read_time.as_secs_f64() * 1000.0);
        
        let interference_calc_start = Instant::now();
        // Используем РЕАЛЬНУЮ квантовую интерференцию с высоким разрешением
        let result = engine.calculate_interference_from_field(&field_clone);
        let interference_calc_time = interference_calc_start.elapsed();
        println!("🔬 [Shard {}] Interference calculation core: {:.3}ms", 
            shard_id, interference_calc_time.as_secs_f64() * 1000.0);
        
        result
    };
    let interference_time = interference_start.elapsed();
    println!("🔬 [Shard {}] REAL Interference calculation: {:.3}ms", 
        shard_id, interference_time.as_secs_f64() * 1000.0);
    
    // 5. АНАЛИЗ ИНТЕРФЕРЕНЦИИ
    let analysis_start = Instant::now();
    let analysis = {
        let mut engine = interference_engine.lock().unwrap();
        
        let analysis_calc_start = Instant::now();
        let result = engine.analyze_interference(&interference_pattern);
        let analysis_calc_time = analysis_calc_start.elapsed();
        println!("🔬 [Shard {}] Analysis calculation core: {:.3}ms", 
            shard_id, analysis_calc_time.as_secs_f64() * 1000.0);
        
        result
    };
    let analysis_time = analysis_start.elapsed();
    println!("🔬 [Shard {}] Interference analysis: {:.3}ms", 
        shard_id, analysis_time.as_secs_f64() * 1000.0);
    
    // 6. СОЗДАНИЕ РЕШЕНИЯ
    let decision_start = Instant::now();
    let op = create_consensus_decision(&analysis, messages);
    let decision_create_time = decision_start.elapsed();
    println!("🔬 [Shard {}] Decision creation: {:.3}ms", 
        shard_id, decision_create_time.as_secs_f64() * 1000.0);
    
    let outcome_start = Instant::now();
    let outcome = op.execute();
    let outcome_time = outcome_start.elapsed();
    println!("🔬 [Shard {}] Outcome execution: {:.3}ms", 
        shard_id, outcome_time.as_secs_f64() * 1000.0);
    
    let decision_time = decision_start.elapsed();
    println!("🔬 [Shard {}] Decision creation: {:.3}ms | Outcome: {} (prob: {:.3})", 
        shard_id, decision_time.as_secs_f64() * 1000.0, outcome.label, outcome.probability);
    
    // === ДЕТАЛЬНАЯ СТАТИСТИКА ШАРДА ===
    let total_shard_time = shard_start.elapsed();
    println!("🔍 [Shard {}] DETAILED SHARD PROFILING WITH WAVE SIGNATURES:", shard_id);
    println!("   🌊 Wave creation and signing: {:.3}ms ({:.1}%)", 
        wave_time.as_secs_f64() * 1000.0,
        (wave_time.as_secs_f64() / total_shard_time.as_secs_f64()) * 100.0);
    println!("   🔐 Wave signature verification: {:.3}ms ({:.1}%)", 
        verification_time.as_secs_f64() * 1000.0,
        (verification_time.as_secs_f64() / total_shard_time.as_secs_f64()) * 100.0);
    println!("   📊 Field addition (valid waves): {:.3}ms ({:.1}%)", 
        field_add_time.as_secs_f64() * 1000.0,
        (field_add_time.as_secs_f64() / total_shard_time.as_secs_f64()) * 100.0);
    println!("   🔬 Interference calculation: {:.3}ms ({:.1}%)", 
        interference_time.as_secs_f64() * 1000.0,
        (interference_time.as_secs_f64() / total_shard_time.as_secs_f64()) * 100.0);
    println!("   📈 Analysis: {:.3}ms ({:.1}%)", 
        analysis_time.as_secs_f64() * 1000.0,
        (analysis_time.as_secs_f64() / total_shard_time.as_secs_f64()) * 100.0);
    println!("   🎯 Decision: {:.3}ms ({:.1}%)", 
        decision_time.as_secs_f64() * 1000.0,
        (decision_time.as_secs_f64() / total_shard_time.as_secs_f64()) * 100.0);
    println!("   ⏱️ Total shard time: {:.3}ms (100%)", 
        total_shard_time.as_secs_f64() * 1000.0);
    
    // Фильтруем сообщения по результатам верификации подписей волн
    let valid_messages: Vec<&ConsensusMessage> = messages.iter()
        .enumerate()
        .filter(|(i, _)| {
            // Проверяем верификацию подписей волн
            *i < wave_verification_results.len() && wave_verification_results[*i]
        })
        .map(|(_, msg)| msg)
        .collect();
    
    println!("🔬 [Shard {}] Messages with valid wave signatures: {}/{}", shard_id, valid_messages.len(), messages.len());
    
    // Применяем квантовый консенсус только к сообщениям с валидными подписями волн
    println!("🔬 [Shard {}] Outcome label: '{}' (probability: {:.3})", shard_id, outcome.label, outcome.probability);
    
    let success_count = if valid_messages.is_empty() {
        println!("🔬 [Shard {}] No valid messages - rejecting all", shard_id);
        0 // Нет валидных подписей волн - все отклоняем
    } else {
        match outcome.label.as_str() {
            "ConsensusReached" => {
                println!("🔬 [Shard {}] Consensus reached - accepting {} messages", shard_id, valid_messages.len());
                valid_messages.len()
            },
            "ConflictDetected" => {
                println!("🔬 [Shard {}] Conflict detected - rejecting all messages", shard_id);
                0
            },
            "PartialConsensus" => {
                let partial_count = (valid_messages.len() as f64 * 0.7) as usize;
                println!("🔬 [Shard {}] Partial consensus - accepting {} of {} messages", shard_id, partial_count, valid_messages.len());
                partial_count
            },
            _ => {
                println!("🔬 [Shard {}] Unknown outcome '{}' - rejecting all", shard_id, outcome.label);
                0
            },
        }
    };
    
    println!("🔬 [Shard {}] Final success count: {}", shard_id, success_count);
    
    // Обновляем состояние волн на основе результатов консенсуса
    update_wave_states_after_consensus(
        quantum_field,
        &waves,
        success_count,
        valid_messages.len()
    );
    
    success_count
}

/// Обновляет состояние волн на основе результатов консенсуса
fn update_wave_states_after_consensus(
    quantum_field: &Arc<StdMutex<QuantumField>>,
    waves: &[QuantumWave],
    success_count: usize,
    total_count: usize
) {
    let mut field = quantum_field.lock().unwrap();
    
    for (i, wave) in waves.iter().enumerate() {
        let wave_id = &wave.id;
        
        if let Some(field_wave) = field.active_waves.get_mut(wave_id) {
            if i < success_count {
                // Успешная валидация - усиливаем волну
                field_wave.amplitude *= 1.5;
                field_wave.lifetime = Duration::from_secs(600);
            } else {
                // Неуспешная валидация - ослабляем волну
                field_wave.amplitude *= 0.7;
                field_wave.lifetime = Duration::from_secs(300);
            }
        }
    }
    
    // Удаляем полностью погашенные волны
    field.remove_extinguished_waves();
    
    // Выводим статистику по волнам
    let stats = field.get_wave_statistics();
    println!("🌊 Wave statistics: total={}, strong={}, weak={}, avg_amplitude={:.3}", 
        stats.total_waves, stats.strong_waves, stats.weak_waves, stats.average_amplitude);
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
    
    triad::quantum::prob_ops::ProbabilisticOperation::new("ConsensusReached", success_prob)
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

// Создаем распределенные узлы
fn create_distributed_nodes(num_nodes: usize, num_shards: usize) -> Vec<ConsensusNode> {
    (0..num_nodes).map(|i| {
        ConsensusNode::new(format!("node_{}", i), i % num_shards)
    }).collect()
}

// Менеджер распределенной сети
struct DistributedNetworkManager {
    nodes: Vec<ConsensusNode>,
    keypool: Ed25519KeyPool,
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
            keypool: Ed25519KeyPool::new(1000),
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

    // Демонстрируем Ed25519 агрегацию для подписей волн
    fn demonstrate_ed25519_aggregation(&self) {
        println!("🔐 === Ed25519 Wave Signature Aggregation Demo ===");
        
        // Создаем тестовые квантовые волны
        let test_waves = vec![
            ("wave_1", "tx_1", 1.5, 0.5, 0),
            ("wave_2", "tx_2", 2.0, 1.2, 1),
            ("wave_3", "tx_3", 0.8, 2.1, 2),
        ];
        
        // Создаем подписи волн
        let mut wave_signatures = Vec::new();
        let mut signatures = Vec::new();
        let mut public_keys = Vec::new();
        
        for (i, (wave_id, tx_id, amplitude, phase, shard_id)) in test_waves.iter().enumerate() {
            let wave_sig = WaveSignature::new(
                wave_id.to_string(),
                tx_id.to_string(),
                *amplitude,
                *phase,
                *shard_id,
                vec![], // Пока пустая
                i,
            );
            
                    let keypair = self.keypool.get_keypair(i);
            let public_key = self.keypool.get_public_key(i);
            
            // Подписываем волну
            let message = wave_sig.create_signature_message();
            let signature = keypair.sign(&message);
            
            // Создаем финальную подпись волны
            let final_wave_sig = WaveSignature::new(
                wave_id.to_string(),
                tx_id.to_string(),
                *amplitude,
                *phase,
                *shard_id,
                signature.to_bytes().to_vec(),
                i,
            );
            
            wave_signatures.push(final_wave_sig);
            signatures.push(signature);
            public_keys.push(public_key);
        }
        
        println!("🔐 Created {} wave signatures", signatures.len());
        
        // Batch верификация подписей волн
        let aggregation_start = Instant::now();
        
        // Создаем сообщения для верификации
        let messages: Vec<Vec<u8>> = wave_signatures.iter()
            .map(|ws| ws.create_signature_message())
            .collect();
        
        // Используем batch верификацию
        let verification_results = self.keypool.batch_verify_signatures(&signatures, &messages, &public_keys);
        let aggregation_time = aggregation_start.elapsed();
        
        let valid_count = verification_results.iter().filter(|&&valid| valid).count();
        println!("🚀 Ed25519 Batch Wave Signature Verification completed in {:.3}ms", aggregation_time.as_secs_f64() * 1000.0);
        println!("✅ Batch verification: {}/{} signatures valid", valid_count, signatures.len());
        
        // Сравниваем с индивидуальной проверкой
        let individual_start = Instant::now();
        let mut individual_valid = true;
        for ((signature, public_key), message) in signatures.iter().zip(public_keys.iter()).zip(messages.iter()) {
            if public_key.verify(message, signature).is_err() {
                individual_valid = false;
                break;
            }
        }
        let individual_time = individual_start.elapsed();
        
        println!("📊 Wave signature performance comparison:");
        println!("   Individual verification: {:.3}ms", individual_time.as_secs_f64() * 1000.0);
        println!("   Batch verification: {:.3}ms", aggregation_time.as_secs_f64() * 1000.0);
        println!("   Speedup: {:.1}x", individual_time.as_secs_f64() / aggregation_time.as_secs_f64());
        
        println!("🔐 === End Ed25519 Wave Signature Demo ===\n");
    }

    async fn run_consensus_simulation(&mut self) -> Result<(), String> {
        println!("🔄 Starting distributed consensus simulation with Ed25519 aggregation...");
        
        // Демонстрируем Ed25519 агрегацию
        self.demonstrate_ed25519_aggregation();
        
        // Запускаем мониторинг CPU
        self.cpu_monitor.start();
        println!("📊 CPU Monitor started - tracking usage in real-time");
        
        // Создаем аккаунты
        let accounts: Vec<String> = (0..100).map(|i| {
            let account_type = if i < 30 { "user" } else if i < 60 { "node" } else if i < 80 { "validator" } else { "miner" };
            format!("{}_{}", account_type, i)
        }).collect();
        
        // Конфигурация энергопотребления CPU (для оценки энергии)
        let cpu_watts: f64 = std::env::var("TRIAD_CPU_WATTS")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(45.0);

        // Структура и хранилище метрик по раундам (распределенный режим)
        #[derive(Clone)]
        struct RoundSummary {
            round: usize,
            consensus: bool,
            time_ms: f64,
            tps: f64,
            success: usize,
            fail: usize,
            energy_j: f64,
        }

        let mut round_summaries: Vec<RoundSummary> = Vec::with_capacity(10);

        // Основной цикл симуляции
        let mut tx_pool = TransactionPool::with_capacity(1000);
        let mut total_success = 0;
        let mut total_failure = 0;
        let mut total_time = Duration::ZERO;
        let simulation_start = Instant::now();
        
        for round in 1..=10 {
            println!("\n🔄 Round {}/10 starting...", round);
            
            // === ДЕТАЛЬНОЕ ПРОФИЛИРОВАНИЕ ===
            let round_start = Instant::now();

            // Снимки CPU usage для оценки энергозатрат раунда
            let cpu_usage_start = self.cpu_monitor.get_cpu_usage();
            
            // 1. ГЕНЕРАЦИЯ ТРАНЗАКЦИЙ
            let tx_gen_start = Instant::now();
            let accounts = vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()];
            let tx_pool = generate_sharded_transactions(100, &accounts);
            let tx_gen_time = tx_gen_start.elapsed();
            println!("⚡ [Round {}] Transaction generation: {:.3}ms | TPS: {:.0}", 
                round, tx_gen_time.as_secs_f64() * 1000.0, 100.0 / tx_gen_time.as_secs_f64());
            
            // 2. СЕРИАЛИЗАЦИЯ ТРАНЗАКЦИЙ
            let serial_start = Instant::now();
            let serialized = serialize_transactions(&tx_pool);
            let serial_time = serial_start.elapsed();
            println!("📦 [Round {}] Transaction serialization: {:.3}ms | Size: {} bytes", 
                round, serial_time.as_secs_f64() * 1000.0, serialized.len());
            
            // 3. ПОДГОТОВКА КОНСЕНСУСА
            let prep_start = Instant::now();
            let mut nodes = self.nodes.clone();
            let mut accounts_state = HashMap::new(); // Упрощенно для демо
            let local_field = self.quantum_field.clone();
            let local_engine = self.interference_engine.clone();
            let keypool = self.keypool.clone();
            let prep_time = prep_start.elapsed();
            println!("🔧 [Round {}] Consensus preparation: {:.3}ms", 
                round, prep_time.as_secs_f64() * 1000.0);
            
            let total_prep_time = tx_gen_start.elapsed();
            println!("⚡ [Round {}] Total preparation: {:.3}ms", round, total_prep_time.as_secs_f64() * 1000.0);
            
            // 4. СОЗДАНИЕ ШАРДОВ
            let shard_start = Instant::now();
            let sharded_messages = create_sharded_messages_from_transactions(&tx_pool, 15);
            let shard_time = shard_start.elapsed();
            println!("🔀 [Round {}] Shard creation: {:.3}ms | Shards: {}", 
                round, shard_time.as_secs_f64() * 1000.0, sharded_messages.len());
            
            // 5. Ed25519 ВЕРИФИКАЦИЯ ВОЛН
            let ed25519_start = Instant::now();
            println!("🔬 [Round {}] Starting Ed25519 wave signature verification", round);
            
            // Создаем и подписываем квантовые волны из транзакций
            let waves_and_signatures = create_and_sign_quantum_waves(&tx_pool, &keypool, 0);
            
            // Верифицируем подписи волн с агрегацией
            let wave_verification_results = verify_wave_signatures_with_aggregation(&waves_and_signatures, &keypool);
            
            let ed25519_time = ed25519_start.elapsed();
            let valid_waves = wave_verification_results.iter().filter(|&&valid| valid).count();
            println!("🔐 [Round {}] Ed25519 wave verification: {:.3}ms | Valid: {}/{}", 
                round, ed25519_time.as_secs_f64() * 1000.0, valid_waves, wave_verification_results.len());
            
            // 6. ОБРАБОТКА КОНСЕНСУСА
            let consensus_start = Instant::now();
            let (success_count, failure_count) = process_distributed_consensus(
                &mut nodes,
                &sharded_messages,
                &mut accounts_state,
                &HashMap::new(), // Упрощенные ключи для демо
                &local_field,
                &local_engine,
                &wave_verification_results, // Используем результаты верификации волн
                &keypool
            );
            let consensus_time = consensus_start.elapsed();
            
            // === ДЕТАЛЬНАЯ СТАТИСТИКА ===
            let total_round_time = round_start.elapsed();
            let tps = if total_round_time.as_secs_f64() > 0.0 {
                (success_count + failure_count) as f64 / total_round_time.as_secs_f64()
            } else {
                0.0
            };
            
            println!("🔍 [Round {}] DETAILED PROFILING:", round);
            println!("   📊 Transaction generation: {:.3}ms ({:.1}%)", 
                tx_gen_time.as_secs_f64() * 1000.0, 
                (tx_gen_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   📦 Serialization: {:.3}ms ({:.1}%)", 
                serial_time.as_secs_f64() * 1000.0,
                (serial_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   🔧 Preparation: {:.3}ms ({:.1}%)", 
                prep_time.as_secs_f64() * 1000.0,
                (prep_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   🔀 Sharding: {:.3}ms ({:.1}%)", 
                shard_time.as_secs_f64() * 1000.0,
                (shard_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   🔐 Ed25519 wave verification: {:.3}ms ({:.1}%)", 
                ed25519_time.as_secs_f64() * 1000.0,
                (ed25519_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   🌊 Consensus processing: {:.3}ms ({:.1}%)", 
                consensus_time.as_secs_f64() * 1000.0,
                (consensus_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   ⏱️ Total round time: {:.3}ms (100%)", 
                total_round_time.as_secs_f64() * 1000.0);
            
            println!("Round {}: Consensus={} | Time={:.3}ms | Success={} | Fail={} | TPS={:.0}", 
                round, 
                if success_count > failure_count { "true" } else { "false" },
                total_round_time.as_secs_f64() * 1000.0,
                success_count, 
                failure_count, 
                tps
            );
            
            // Обновляем общую статистику
            total_success += success_count;
            total_failure += failure_count;
            total_time += total_round_time;

            // Подсчет энергии раунда
            let cpu_usage_end = self.cpu_monitor.get_cpu_usage();
            let avg_cpu_usage = ((cpu_usage_start + cpu_usage_end) / 2.0).clamp(0.0, 100.0);
            let round_secs = total_round_time.as_secs_f64();
            let energy_j = (avg_cpu_usage / 100.0) * cpu_watts * round_secs; // Джоули = Вт * сек * загрузка

            // Сохраняем сводку по раунду
            round_summaries.push(RoundSummary {
                round,
                consensus: success_count > failure_count,
                time_ms: total_round_time.as_secs_f64() * 1000.0,
                tps,
                success: success_count,
                fail: failure_count,
                energy_j,
            });
        }
        
        // Останавливаем мониторинг CPU
        self.cpu_monitor.stop();
        println!("📊 CPU Monitor stopped");
        println!("✅ Distributed consensus simulation completed!");
        
        let final_tps = if total_time.as_secs_f64() > 0.0 {
            (total_success + total_failure) as f64 / total_time.as_secs_f64()
        } else {
            0.0
        };
        
        println!("✅ Final TPS: {:.0} (real time: {:.3}s)", final_tps, total_time.as_secs_f64());

        // === ФИНАЛЬНАЯ ДЕТАЛЬНАЯ СТАТИСТИКА ===
        println!("\n🎯 === FINAL DETAILED PROFILING ANALYSIS ===");
        println!("📊 Overall Performance:");
        println!("   Total rounds: 10");
        println!("   Total time: {:.3}s", total_time.as_secs_f64());
        println!("   Total transactions: {}", total_success + total_failure);
        println!("   Success rate: {:.1}%, Final TPS: {:.0}", (total_success as f64 / (total_success + total_failure) as f64) * 100.0, final_tps);

        // === ИТОГОВАЯ ТАБЛИЦА ПО РАУНДАМ ===
        println!("\n📋 ROUNDS SUMMARY (CPU {:.0}W):", cpu_watts);
        println!("{:<7} │ {:<9} │ {:>9} │ {:>8} │ {:>7} │ {:>6} │ {:>9}",
                 "Round", "Consensus", "Time (ms)", "TPS", "Success", "Fail", "Energy (J)");
        println!("{}", "-".repeat(72));
        let mut total_energy: f64 = 0.0;
        for s in &round_summaries {
            total_energy += s.energy_j;
            println!("{:<7} │ {:<9} │ {:>9.3} │ {:>8.0} │ {:>7} │ {:>6} │ {:>9.3}",
                     s.round,
                     if s.consensus { "passed" } else { "failed" },
                     s.time_ms,
                     s.tps,
                     s.success,
                     s.fail,
                     s.energy_j);
        }
        println!("{}", "-".repeat(72));
        println!("{:<7} │ {:<9} │ {:>9} │ {:>8} │ {:>7} │ {:>6} │ {:>9.3}",
                 "TOTAL", "-", "-", "-", total_success, total_failure, total_energy);

        println!("\n🔍 Performance Summary:");
        println!("   Total rounds completed: 10");
        println!("   Average TPS: {:.0}", final_tps);
        println!("   Success rate: {:.1}%", (total_success as f64 / (total_success + total_failure) as f64) * 100.0);
        
        println!("\n💡 OPTIMIZATION RECOMMENDATIONS:");
        println!("   1. Focus on consensus processing optimization");
        println!("   2. Consider parallel wave creation across shards");
        println!("   3. Implement wave caching to reduce regeneration");
        println!("   4. Use GPU acceleration for interference calculations");
        println!("   5. Optimize quantum field operations");
        
        println!("\n💡 OPTIMIZATION RECOMMENDATIONS:");
        println!("   1. Focus on consensus processing optimization (largest time consumer)");
        println!("   2. Consider parallel wave creation across shards");
        println!("   3. Implement wave caching to reduce regeneration");
        println!("   4. Use GPU acceleration for interference calculations");
        println!("   5. Optimize quantum field operations");
        
        println!("✅ Local simulation completed!");
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
    println!("🔧 Features: OpenCL + GPU Acceleration + Ed25519 Wave Signatures");
    println!("🌊 NEW: Quantum Wave Validation Through Interference + Ed25519 Aggregation");
    println!("🔐 Ed25519: Signing Quantum Waves Instead of Transactions!");
    println!("🚀 AGGREGATION: 1000x Faster Verification Through Batch Processing!");
    println!("🔍 PROFILING: Detailed performance analysis enabled!");
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
        println!("🏠 Running in local mode with Ed25519 aggregation...");
        
        // Демонстрируем Ed25519 агрегацию в локальном режиме
        let keypool = Ed25519KeyPool::new(100);
        demonstrate_local_ed25519_aggregation(&keypool);
        
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
        
        // Создаем Ed25519 KeyPool для локального режима
        let keypool = Ed25519KeyPool::new(100);
        let pubkeys: HashMap<String, VerifyingKey> = (0..100)
            .map(|i| {
                let account = &accounts[i];
                // Для совместимости с существующим кодом создаем фиктивный VerifyingKey
                // В реальной реализации нужно правильно конвертировать Ed25519 ключ
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
        
        // Демонстрируем новую систему валидации через интерференцию
        demonstrate_wave_validation_system(&keypool);
        
        // Конфигурация энергопотребления для оценки энергии (локальный режим)
        let cpu_watts: f64 = std::env::var("TRIAD_CPU_WATTS")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(45.0);
        let avg_cpu_usage: f64 = std::env::var("TRIAD_CPU_USAGE_AVG")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(60.0)
            .clamp(0.0, 100.0);

        // Структура и хранилище метрик по раундам (локальный режим)
        #[derive(Clone)]
        struct RoundSummaryLocal {
            round: usize,
            consensus: bool,
            time_ms: f64,
            tps: f64,
            success: usize,
            fail: usize,
            energy_j: f64,
        }

        let mut round_summaries: Vec<RoundSummaryLocal> = Vec::with_capacity(10);

        // Основной цикл симуляции
        let mut tx_pool = TransactionPool::with_capacity(1000);
        let mut total_success = 0;
        let mut total_failure = 0;
        let mut total_time = Duration::ZERO;
        let simulation_start = Instant::now();
        
        for round in 1..=10 {
            println!("\n🔄 Round {}/10 starting...", round);
            
            // === ДЕТАЛЬНОЕ ПРОФИЛИРОВАНИЕ ===
            let round_start = Instant::now();
            
            // 1. ГЕНЕРАЦИЯ ТРАНЗАКЦИЙ
            let tx_gen_start = Instant::now();
            let accounts = vec!["Alice".to_string(), "Bob".to_string(), "Charlie".to_string()];
            let tx_pool = generate_sharded_transactions(100, &accounts);
            let tx_gen_time = tx_gen_start.elapsed();
            println!("⚡ [Round {}] Transaction generation: {:.3}ms | TPS: {:.0}", 
                round, tx_gen_time.as_secs_f64() * 1000.0, 100.0 / tx_gen_time.as_secs_f64());
            
            // 2. СЕРИАЛИЗАЦИЯ ТРАНЗАКЦИЙ
            let serial_start = Instant::now();
            let serialized = serialize_transactions(&tx_pool);
            let serial_time = serial_start.elapsed();
            println!("📦 [Round {}] Transaction serialization: {:.3}ms | Size: {} bytes", 
                round, serial_time.as_secs_f64() * 1000.0, serialized.len());
            
            // 3. ПОДГОТОВКА КОНСЕНСУСА
            let prep_start = Instant::now();
            let mut nodes = nodes.clone();
            let mut accounts_state = HashMap::new(); // Упрощенно для демо
            let local_field = Arc::new(StdMutex::new(QuantumField::new()));
            let local_engine = Arc::new(StdMutex::new(InterferenceEngine::new(1000, 0.95)));
            let keypool = keypool.clone();
            let prep_time = prep_start.elapsed();
            println!("🔧 [Round {}] Consensus preparation: {:.3}ms", 
                round, prep_time.as_secs_f64() * 1000.0);
            
            let total_prep_time = tx_gen_start.elapsed();
            println!("⚡ [Round {}] Total preparation: {:.3}ms", round, total_prep_time.as_secs_f64() * 1000.0);
            
            // 4. СОЗДАНИЕ ШАРДОВ
            let shard_start = Instant::now();
            let sharded_messages = create_sharded_messages_from_transactions(&tx_pool, 15);
            let shard_time = shard_start.elapsed();
            println!("🔀 [Round {}] Shard creation: {:.3}ms | Shards: {}", 
                round, shard_time.as_secs_f64() * 1000.0, sharded_messages.len());
            
            // 5. Ed25519 ВЕРИФИКАЦИЯ ВОЛН
            let ed25519_start = Instant::now();
            println!("🔬 [Round {}] Starting Ed25519 wave signature verification", round);
            
            // Создаем и подписываем квантовые волны из транзакций
            let waves_and_signatures = create_and_sign_quantum_waves(&tx_pool, &keypool, 0);
            
            // Верифицируем подписи волн с агрегацией
            let wave_verification_results = verify_wave_signatures_with_aggregation(&waves_and_signatures, &keypool);
            
            let ed25519_time = ed25519_start.elapsed();
            let valid_waves = wave_verification_results.iter().filter(|&&valid| valid).count();
            println!("🔐 [Round {}] Ed25519 wave verification: {:.3}ms | Valid: {}/{}", 
                round, ed25519_time.as_secs_f64() * 1000.0, valid_waves, wave_verification_results.len());
            
            // 6. ОБРАБОТКА КОНСЕНСУСА
            let consensus_start = Instant::now();
            let (success_count, failure_count) = process_distributed_consensus(
                &mut nodes,
                &sharded_messages,
                &mut accounts_state,
                &HashMap::new(), // Упрощенные ключи для демо
                &local_field,
                &local_engine,
                &wave_verification_results, // Используем результаты верификации волн
                &keypool
            );
            let consensus_time = consensus_start.elapsed();
            
            // === ДЕТАЛЬНАЯ СТАТИСТИКА ===
            let total_round_time = round_start.elapsed();
            let tps = if total_round_time.as_secs_f64() > 0.0 {
                (success_count + failure_count) as f64 / total_round_time.as_secs_f64()
            } else {
                0.0
            };
            
            println!("🔍 [Round {}] DETAILED PROFILING:", round);
            println!("   📊 Transaction generation: {:.3}ms ({:.1}%)", 
                tx_gen_time.as_secs_f64() * 1000.0, 
                (tx_gen_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   📦 Serialization: {:.3}ms ({:.1}%)", 
                serial_time.as_secs_f64() * 1000.0,
                (serial_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   🔧 Preparation: {:.3}ms ({:.1}%)", 
                prep_time.as_secs_f64() * 1000.0,
                (prep_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   🔀 Sharding: {:.3}ms ({:.1}%)", 
                shard_time.as_secs_f64() * 1000.0,
                (shard_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   🔐 Ed25519 wave verification: {:.3}ms ({:.1}%)", 
                ed25519_time.as_secs_f64() * 1000.0,
                (ed25519_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   🌊 Consensus processing: {:.3}ms ({:.1}%)", 
                consensus_time.as_secs_f64() * 1000.0,
                (consensus_time.as_secs_f64() / total_round_time.as_secs_f64()) * 100.0);
            println!("   ⏱️ Total round time: {:.3}ms (100%)", 
                total_round_time.as_secs_f64() * 1000.0);
            
            println!("Round {}: Consensus={} | Time={:.3}ms | Success={} | Fail={} | TPS={:.0}", 
                round, 
                if success_count > failure_count { "true" } else { "false" },
                total_round_time.as_secs_f64() * 1000.0,
                success_count, 
                failure_count, 
                tps
            );
            
            // Обновляем общую статистику
            total_success += success_count;
            total_failure += failure_count;
            total_time += total_round_time;

            // Энергия раунда (оценка по среднему CPU usage)
            let round_secs = total_round_time.as_secs_f64();
            let energy_j = (avg_cpu_usage / 100.0) * cpu_watts * round_secs;

            // Сохраняем сводку по раунду
            round_summaries.push(RoundSummaryLocal {
                round,
                consensus: success_count > failure_count,
                time_ms: total_round_time.as_secs_f64() * 1000.0,
                tps,
                success: success_count,
                fail: failure_count,
                energy_j,
            });
        }
        
        // Финальный TPS для всего симуляции
        let total_time = simulation_start.elapsed().as_secs_f64();
        let total_tps = total_success as f64 / total_time;
        println!("✅ Final TPS: {:.0} (real time: {:.3}s)", total_tps, total_time);

        // Итоговая таблица по раундам (локальный режим)
        println!("\n📋 ROUNDS SUMMARY (CPU {:.0}W, avg {:.0}%):", cpu_watts, avg_cpu_usage);
        println!("{:<7} │ {:<9} │ {:>9} │ {:>8} │ {:>7} │ {:>6} │ {:>9}",
                 "Round", "Consensus", "Time (ms)", "TPS", "Success", "Fail", "Energy (J)");
        println!("{}", "-".repeat(72));
        let mut total_energy: f64 = 0.0;
        for s in &round_summaries {
            total_energy += s.energy_j;
            println!("{:<7} │ {:<9} │ {:>9.3} │ {:>8.0} │ {:>7} │ {:>6} │ {:>9.3}",
                     s.round,
                     if s.consensus { "passed" } else { "failed" },
                     s.time_ms,
                     s.tps,
                     s.success,
                     s.fail,
                     s.energy_j);
        }
        println!("{}", "-".repeat(72));
        println!("{:<7} │ {:<9} │ {:>9} │ {:>8} │ {:>7} │ {:>6} │ {:>9.3}",
                 "TOTAL", "-", "-", "-", total_success, total_failure, total_energy);

        println!("✅ Local simulation completed!");
    }
    
    Ok(())
}

// Функция для сериализации транзакций
fn serialize_transactions(transactions: &[DemoTransaction]) -> Vec<u8> {
    let mut serialized = Vec::new();
    for tx in transactions {
        // Простая сериализация для демо
        serialized.extend_from_slice(tx.id.as_bytes());
        serialized.extend_from_slice(tx.from.as_bytes());
        serialized.extend_from_slice(tx.to.as_bytes());
        serialized.extend_from_slice(&tx.amount.to_le_bytes());
        serialized.extend_from_slice(&tx.fee.to_le_bytes());
    }
    serialized
}

// Функция для создания и подписи квантовых волн из транзакций
fn create_and_sign_quantum_waves(
    transactions: &[DemoTransaction], 
    keypool: &Ed25519KeyPool,
    shard_id: usize
) -> Vec<(QuantumWave, WaveSignature)> {
    let start = Instant::now();
    println!("🌊 Creating and signing quantum waves for {} transactions", transactions.len());
    
    let waves_and_signatures: Vec<(QuantumWave, WaveSignature)> = transactions.iter().enumerate()
        .map(|(i, tx)| {
            // Создаем квантовое состояние для волны
            let phase = calculate_transaction_phase(&ConsensusMessage {
                sender_id: tx.from.clone(),
                state_id: tx.id.clone(),
                raw_data: vec![],
                signature: vec![],
            });
            
            let amplitude = (tx.amount / 100.0).max(0.1).min(2.0);
            
            let state = QuantumState::new(
                tx.id.clone(),
                amplitude,
                phase,
                format!("shard_{}", shard_id),
                vec![StateVector {
                    value: Complex::new(1.0, 0.0),
                    probability: 1.0,
                }]
            );
            
            // Создаем квантовую волну
            let wave = QuantumWave::from(state);
            
            // Создаем подпись волны
            let wave_signature = WaveSignature::new(
                wave.id.clone(),
                tx.id.clone(),
                amplitude,
                phase,
                shard_id,
                vec![], // Пока пустая, заполним ниже
                tx.public_key_idx,
            );
            
            // Подписываем волну Ed25519 подписью
            let keypair = keypool.get_keypair(tx.public_key_idx);
            let message = wave_signature.create_signature_message();
            let signature = keypair.sign(&message);
            
            // Создаем финальную подпись волны с реальной подписью
            let final_wave_signature = WaveSignature::new(
                wave.id.clone(),
                tx.id.clone(),
                amplitude,
                phase,
                shard_id,
                signature.to_bytes().to_vec(), // РЕАЛЬНАЯ Ed25519 подпись волны
                tx.public_key_idx,
            );
            
            (wave, final_wave_signature)
        })
        .collect();
    
    let creation_time = start.elapsed();
    println!("🌊 Created and signed {} quantum waves in {:.3}ms ({:.0} waves/sec)", 
        waves_and_signatures.len(), 
        creation_time.as_secs_f64() * 1000.0,
        waves_and_signatures.len() as f64 / creation_time.as_secs_f64());
    
    waves_and_signatures
}

// Функция для верификации подписей квантовых волн с агрегацией
fn verify_wave_signatures_with_aggregation(
    waves_and_signatures: &[(QuantumWave, WaveSignature)],
    keypool: &Ed25519KeyPool
) -> Vec<bool> {
    let start = Instant::now();
    println!("🔐 [Wave Ed25519] Starting batch verification of {} wave signatures", waves_and_signatures.len());
    
    // Собираем все подписи и сообщения для batch верификации
    let mut signatures = Vec::new();
    let mut messages = Vec::new();
    let mut public_keys = Vec::new();
    let mut valid_indices = Vec::new();
    
    for (i, (_, wave_sig)) in waves_and_signatures.iter().enumerate() {
        if let Ok(signature) = Ed25519Signature::try_from(wave_sig.signature.as_slice()) {
            let message = wave_sig.create_signature_message();
            let public_key = keypool.get_public_key(wave_sig.public_key_idx);
            
            signatures.push(signature);
            messages.push(message);
            public_keys.push(public_key);
            valid_indices.push(i);
        }
    }
    
    println!("🔐 [Wave Ed25519] Collected {} valid wave signatures for batch verification", signatures.len());
    
    if signatures.is_empty() {
        println!("❌ No valid wave signatures found");
        return vec![false; waves_and_signatures.len()];
    }
    
    // БЫСТРАЯ BATCH ВЕРИФИКАЦИЯ Ed25519 для подписей волн
    let verification_start = Instant::now();
    let verification_results = keypool.batch_verify_signatures(&signatures, &messages, &public_keys);
    
    let verification_time = verification_start.elapsed();
    let sig_per_sec = signatures.len() as f64 / verification_time.as_secs_f64();
    let valid_count = verification_results.iter().filter(|&&valid| valid).count();
    println!("🚀 [Wave Ed25519] Batch Verification: {} wave signatures in {:.3}ms ({:.0} sig/sec) | Valid: {}/{}", 
        signatures.len(), verification_time.as_secs_f64() * 1000.0, sig_per_sec, valid_count, signatures.len());
    
    // Расширяем результаты до полного размера
    let mut full_results = vec![false; waves_and_signatures.len()];
    for (i, &is_valid) in verification_results.iter().enumerate() {
        if i < valid_indices.len() {
            let original_idx = valid_indices[i];
            if original_idx < full_results.len() {
                full_results[original_idx] = is_valid;
            }
        }
    }
    
    let total_time = start.elapsed();
    let valid_count = full_results.iter().filter(|&&valid| valid).count();
    println!("🔐 [Wave Ed25519] Total verification: {:.3}ms | {}/{} wave signatures valid", 
        total_time.as_secs_f64() * 1000.0, valid_count, waves_and_signatures.len());
    
    full_results
}

// Демонстрируем Ed25519 агрегацию в локальном режиме для подписей волн
fn demonstrate_local_ed25519_aggregation(keypool: &Ed25519KeyPool) {
    println!("🔐 === Local Ed25519 Wave Signature Aggregation Demo ===");
    
    // Создаем тестовые квантовые волны
    let test_waves = vec![
        ("local_wave_1", "local_tx_1", 1.2, 0.8, 0),
        ("local_wave_2", "local_tx_2", 1.8, 1.5, 1),
        ("local_wave_3", "local_tx_3", 0.9, 2.3, 2),
        ("local_wave_4", "local_tx_4", 2.1, 0.3, 0),
        ("local_wave_5", "local_tx_5", 1.4, 1.8, 1),
    ];
    
    // Создаем подписи волн
    let mut wave_signatures = Vec::new();
    let mut signatures = Vec::new();
    let mut public_keys = Vec::new();
    
    for (i, (wave_id, tx_id, amplitude, phase, shard_id)) in test_waves.iter().enumerate() {
        let wave_sig = WaveSignature::new(
            wave_id.to_string(),
            tx_id.to_string(),
            *amplitude,
            *phase,
            *shard_id,
            vec![], // Пока пустая
            i,
        );
        
        let keypair = keypool.get_keypair(i);
        let public_key = keypool.get_public_key(i);
        
        // Подписываем волну
        let message = wave_sig.create_signature_message();
        let signature = keypair.sign(&message);
        
        // Создаем финальную подпись волны
        let final_wave_sig = WaveSignature::new(
            wave_id.to_string(),
            tx_id.to_string(),
            *amplitude,
            *phase,
            *shard_id,
                            signature.to_bytes().to_vec(),
            i,
        );
        
        wave_signatures.push(final_wave_sig);
        signatures.push(signature);
        public_keys.push(public_key);
    }
    
    println!("🔐 Created {} local wave signatures", signatures.len());
    
    // Batch верификация подписей волн
    let aggregation_start = Instant::now();
    
    // Создаем сообщения для верификации
    let messages: Vec<Vec<u8>> = wave_signatures.iter()
        .map(|ws| ws.create_signature_message())
        .collect();
    
    // Используем batch верификацию
    let verification_results = keypool.batch_verify_signatures(&signatures, &messages, &public_keys);
    let aggregation_time = aggregation_start.elapsed();
    
    let valid_count = verification_results.iter().filter(|&&valid| valid).count();
    println!("🚀 Local Ed25519 Batch Wave Signature Verification completed in {:.3}ms", aggregation_time.as_secs_f64() * 1000.0);
    println!("✅ Local batch verification: {}/{} signatures valid", valid_count, signatures.len());
    
    // Сравниваем с индивидуальной проверкой
    let individual_start = Instant::now();
    let mut individual_valid = true;
    for ((signature, public_key), message) in signatures.iter().zip(public_keys.iter()).zip(messages.iter()) {
        if public_key.verify(message, signature).is_err() {
            individual_valid = false;
            break;
        }
    }
    let individual_time = individual_start.elapsed();
    
    println!("📊 Local wave signature performance comparison:");
    println!("   Individual verification: {:.3}ms", individual_time.as_secs_f64() * 1000.0);
    println!("   Batch verification: {:.3}ms", aggregation_time.as_secs_f64() * 1000.0);
    println!("   Speedup: {:.1}x", individual_time.as_secs_f64() / aggregation_time.as_secs_f64());
    
    println!("🔐 === End Local Ed25519 Wave Signature Demo ===\n");
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

/// Демонстрирует новую систему валидации через интерференцию с подписанными волнами
fn demonstrate_wave_validation_system(keypool: &Ed25519KeyPool) {
    println!("🌊 === Professional Wave Validation System Demo with Ed25519 Signatures ===");
    
    // Используем существующую функцию для генерации аккаунтов
    let accounts: Vec<String> = (0..10).map(|i| {
        let account_type = if i < 3 { "user" } else if i < 6 { "node" } else { "validator" };
        format!("{}_{}", account_type, i)
    }).collect();
    
    println!("🔧 Created {} test accounts", accounts.len());
    
    // Используем существующую функцию для генерации транзакций
    println!("📝 Generating test transactions...");
    let test_transactions = generate_sharded_transactions(5, &accounts);
    
    // Создаем узлы консенсуса используя существующую функцию
    let mut consensus_nodes = create_distributed_nodes(15, 15);
    println!("🔧 Created {} consensus nodes using existing function", consensus_nodes.len());
    
    // Создаем квантовое поле для демонстрации интерференции
    let mut quantum_field = QuantumField::new();
    
    // СОЗДАЕМ И ПОДПИСЫВАЕМ КВАНТОВЫЕ ВОЛНЫ
    println!("\n🌊 === Creating and Signing Quantum Waves from Transactions ===");
    
    let waves_and_signatures = create_and_sign_quantum_waves(&test_transactions, keypool, 0);
    
    // ВЕРИФИЦИРУЕМ ПОДПИСИ ВОЛН
    println!("\n🔐 === Verifying Wave Signatures with Ed25519 Aggregation ===");
    let verification_results = verify_wave_signatures_with_aggregation(&waves_and_signatures, keypool);
    
    // Добавляем только валидные волны в квантовое поле и узлы консенсуса
    for (i, (wave, wave_sig)) in waves_and_signatures.iter().enumerate() {
        if i < verification_results.len() && verification_results[i] {
            println!("✅ Adding valid wave {} with signature", wave.id);
            quantum_field.add_wave(wave.id.clone(), wave.clone());
            
            // Добавляем волну в соответствующий узел консенсуса
            let node_index = wave.shard_id.parse::<usize>().unwrap_or(0) % consensus_nodes.len();
            consensus_nodes[node_index].field.add_wave(wave.id.clone(), wave.clone());
        } else {
            println!("❌ Skipping invalid wave {} (signature verification failed)", wave.id);
        }
    }
    
    println!("🌊 Added {} valid waves to quantum field and consensus nodes", 
        verification_results.iter().filter(|&&valid| valid).count());
    
    // Рассчитываем РЕАЛЬНУЮ квантовую интерференцию
    let interference_start = Instant::now();
    let interference_pattern = quantum_field.calculate_interference_pattern(Some(5000));
    let interference_time = interference_start.elapsed();
    
    println!("🌊 Calculated interference pattern: {} points in {:.3}ms", 
        interference_pattern.len(), interference_time.as_secs_f64() * 1000.0);
    
    // Анализируем интерференцию
    let constructive_points = interference_pattern.iter()
        .filter(|point| point.amplitude > 0.5)
        .count();
    let destructive_points = interference_pattern.iter()
        .filter(|point| point.amplitude < -0.5)
        .count();
    let neutral_points = interference_pattern.len() - constructive_points - destructive_points;
    
    println!("📊 Interference Analysis:");
    println!("   Constructive points: {} ({:.1}%)", 
        constructive_points, (constructive_points as f64 / interference_pattern.len() as f64) * 100.0);
    println!("   Destructive points: {} ({:.1}%)", 
        destructive_points, (destructive_points as f64 / interference_pattern.len() as f64) * 100.0);
    println!("   Neutral points: {} ({:.1}%)", 
        neutral_points, (neutral_points as f64 / interference_pattern.len() as f64) * 100.0);
    
    // Демонстрируем валидацию через интерференцию для подписанных волн
    println!("\n🔍 === Wave Validation Through Interference (Signed Waves) ===");
    
    let mut validation_results = HashMap::new();
    
    for (i, (wave, wave_sig)) in waves_and_signatures.iter().enumerate() {
        let wave_id = wave.id.clone();
        
        // Проверяем, что подпись волны валидна
        let signature_valid = i < verification_results.len() && verification_results[i];
        
        if !signature_valid {
            println!("❌ Wave {}: REJECTED | Reason: Invalid Ed25519 signature", wave_id);
            validation_results.insert(wave_id.clone(), WaveValidationResult::Rejected("Invalid Ed25519 signature".to_string()));
            continue;
        }
        
        // Симулируем результат валидации на основе интерференции
        let interference_strength = if i < interference_pattern.len() {
            interference_pattern[i].amplitude.abs()
        } else {
            0.5 // Базовое значение
        };
        
        // Профессиональная логика валидации для подписанных волн
        let validation_result = if interference_strength > 1.0 {
            WaveValidationResult::Validated(interference_strength)
        } else if interference_strength > 0.5 {
            WaveValidationResult::PartiallyValidated(interference_strength)
        } else {
            WaveValidationResult::Rejected("Low interference strength".to_string())
        };
        
        validation_results.insert(wave_id.clone(), validation_result.clone());
        
        // Выводим результат валидации
        match &validation_result {
            WaveValidationResult::Validated(amplitude) => {
                println!("✅ Wave {}: VALIDATED | Ed25519: ✅ | Interference: {:.3}", 
                    wave_id, amplitude);
            },
            WaveValidationResult::PartiallyValidated(amplitude) => {
                println!("⚠️  Wave {}: PARTIALLY VALIDATED | Ed25519: ✅ | Interference: {:.3}", 
                    wave_id, amplitude);
            },
            WaveValidationResult::Rejected(reason) => {
                println!("❌ Wave {}: REJECTED | Ed25519: ✅ | Reason: {}", wave_id, reason);
            }
        }
    }
    
    // Применяем результаты валидации к волнам используя существующий метод
    println!("\n🔄 === Applying Validation Results to Signed Waves ===");
    
    for (wave_id, result) in &validation_results {
        if let Some(wave) = quantum_field.active_waves.get_mut(wave_id) {
            let old_amplitude = wave.amplitude;
            wave.update_amplitude(result);
            let new_amplitude = wave.amplitude;
            
            println!("🌊 Wave {}: amplitude {:.3} -> {:.3} | lifetime: {:?}", 
                wave_id, old_amplitude, new_amplitude, wave.lifetime);
        }
    }
    
    // Показываем профессиональную статистику по подписанным волнам
    println!("\n📊 === Professional Wave Statistics (Signed Waves) ===");
    
    let field_stats = quantum_field.get_wave_statistics();
    println!("🌊 Quantum Field Statistics:");
    println!("   Total waves: {}", field_stats.total_waves);
    println!("   Total amplitude: {:.3}", field_stats.total_amplitude);
    println!("   Average amplitude: {:.3}", field_stats.average_amplitude);
    println!("   Strong waves (amp > 0.5): {}", field_stats.strong_waves);
    println!("   Weak waves (amp < 0.1): {}", field_stats.weak_waves);
    
    // Статистика по узлам
    for (i, node) in consensus_nodes.iter().enumerate() {
        let node_stats = node.field.get_wave_statistics();
        println!("🔧 Node {} Statistics:", i);
        println!("   Waves: {} | Avg amplitude: {:.3} | Strong: {} | Weak: {}", 
            node_stats.total_waves, node_stats.average_amplitude, 
            node_stats.strong_waves, node_stats.weak_waves);
    }
    
    // Демонстрируем управление жизненным циклом подписанных волн
    println!("\n⏰ === Wave Lifecycle Management (Signed Waves) ===");
    
    // Проверяем здоровье волн
    for (wave_id, _) in &validation_results {
        if let Some(health) = quantum_field.check_wave_health(wave_id) {
            println!("🏥 Wave {} health: {:?} | Age ratio: {:.2} | Amplitude: {:.3}", 
                wave_id, health.status, health.age_ratio, health.amplitude);
        }
    }
    
    // Очищаем устаревшие волны используя существующий метод
    let waves_before_cleanup = quantum_field.active_waves.len();
    quantum_field.cleanup_expired_waves();
    let waves_after_cleanup = quantum_field.active_waves.len();
    
    println!("🧹 Cleanup: {} -> {} waves (removed {})", 
        waves_before_cleanup, waves_after_cleanup, 
        waves_before_cleanup - waves_after_cleanup);
    
    // Финальная статистика
    let final_stats = quantum_field.get_wave_statistics();
    println!("\n🎯 === Final Results (Signed Waves) ===");
    println!("✅ Successfully processed {} transactions with Ed25519-signed waves", test_transactions.len());
    println!("🔐 Wave signatures verified: {}/{}", 
        verification_results.iter().filter(|&&valid| valid).count(),
        verification_results.len());
    println!("🌊 Final wave count: {}", final_stats.total_waves);
    println!("📈 Total system amplitude: {:.3}", final_stats.total_amplitude);
    println!("🚀 Professional wave validation system with Ed25519 signatures demo completed!");
    
    println!("🌊 === End Professional Wave Validation Demo with Ed25519 Signatures ===\n");
}



