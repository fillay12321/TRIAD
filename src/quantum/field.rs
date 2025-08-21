use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::sync::RwLock;
use rayon::prelude::*;
use num_complex::Complex;
use std::time::{Instant, Duration};

// Импорты для Ed25519 подписей квантовых волн
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
#[cfg(feature = "zebra_batch")]
use ed25519_zebra::{VerificationKeyBytes as ZebraVKBytes, Signature as ZebraSig};
#[cfg(feature = "zebra_batch")]
use ed25519_zebra::batch::Verifier as ZebraVerifier;
#[cfg(feature = "zebra_batch")]
use rand07::rngs::OsRng;

#[derive(Debug, Clone)]
pub struct QuantumWave {
    pub id: String,
    pub amplitude: f64,
    pub phase: f64,
    pub shard_id: String,
    pub created_at: Instant,
    pub lifetime: Duration, // Время жизни волны
    pub superposition: Vec<StateVector>,
    pub signature: Vec<u8>, // Ed25519 подпись квантовой волны
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateVector {
    pub value: Complex<f64>,
    pub probability: f64,
}

#[derive(Debug, Clone)]
pub struct QuantumField {
    pub active_waves: HashMap<String, QuantumWave>, // Только активные волны
    pub interference_pattern: Vec<InterferencePoint>,
    cache: Arc<RwLock<HashMap<String, Vec<InterferencePoint>>>>,
    pub max_waves: usize, // Максимальное количество активных волн
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterferencePoint {
    pub position: f64,
    pub amplitude: f64,
    pub phase: f64,
}

impl QuantumField {
    pub fn new() -> Self {
        Self {
            active_waves: HashMap::new(),
            interference_pattern: Vec::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_waves: 100_000, // Убираем искусственное ограничение для масштабируемости
        }
    }

    pub fn add_wave(&mut self, key: String, wave: QuantumWave) {
        // НЕ очищаем устаревшие волны при каждом добавлении - это замедляет работу!
        // self.cleanup_expired_waves(); // Убираем отсюда
        
        // Если достигли лимита, удаляем самую старую волну
        if self.active_waves.len() >= self.max_waves {
            self.remove_oldest_wave();
        }
        
        self.active_waves.insert(key, wave);
        // Убираем автоматическое обновление интерференции - будем вызывать только когда нужно
        // self.update_interference(10_000); // Обновляем интерференцию с высоким разрешением
    }

    /// Безопасно добавляет волну с проверкой подписи (Ed25519)
    pub fn add_signed_wave(&mut self, key: String, wave: QuantumWave, public_key: &VerifyingKey) -> Result<(), String> {
        // Проверяем подпись волны перед добавлением
        if !wave.verify_signature(public_key) {
            return Err("Invalid wave signature".to_string());
        }
        
        // Проверяем, что волна не устарела
        if Instant::now().duration_since(wave.created_at) > wave.lifetime {
            return Err("Wave has expired".to_string());
        }
        
        // Добавляем волну
        self.add_wave(key, wave);
        Ok(())
    }

    /// Добавляет волну без проверки подписи (для внутреннего использования)
    pub fn add_wave_unchecked(&mut self, key: String, wave: QuantumWave) {
        self.add_wave(key, wave);
    }

    /// Проверяет состояние волны
    pub fn check_wave_health(&self, wave_id: &str) -> Option<WaveHealth> {
        if let Some(wave) = self.active_waves.get(wave_id) {
            let age = Instant::now().duration_since(wave.created_at);
            let health_ratio = age.as_secs_f64() / wave.lifetime.as_secs_f64();
            
            let health = if health_ratio < 0.3 {
                WaveHealthStatus::Young
            } else if health_ratio < 0.7 {
                WaveHealthStatus::Mature
            } else if health_ratio < 0.9 {
                WaveHealthStatus::Aging
            } else {
                WaveHealthStatus::Expired
            };
            
            Some(WaveHealth {
                wave_id: wave_id.to_string(),
                amplitude: wave.amplitude,
                age_ratio: health_ratio,
                status: health,
            })
        } else {
            None
        }
    }

    pub fn cleanup_expired_waves(&mut self) {
        let now = Instant::now();
        self.active_waves.retain(|_, wave| {
            now.duration_since(wave.created_at) < wave.lifetime
        });
    }

    pub fn remove_oldest_wave(&mut self) {
        if let Some((oldest_key, _)) = self.active_waves.iter()
            .min_by_key(|(_, wave)| wave.created_at) {
            let key = oldest_key.clone();
            self.active_waves.remove(&key);
        }
    }

    pub fn get_wave(&self, key: &str) -> Result<QuantumWave, String> {
        self.active_waves.get(key)
            .cloned()
            .ok_or_else(|| format!("Wave not found: {}", key))
    }

    /// Параллельная проверка подписей набора волн (batched via rayon)
    /// Принимает кортежи (wave_ref, verifying_key) и возвращает true, если все подписи корректны
    pub fn verify_waves_parallel<'a>(&self, items: &[( &'a QuantumWave, VerifyingKey )]) -> bool {
        items.par_iter().all(|(wave, vk)| wave.verify_signature(vk))
    }

    /// Унифицированная батч-верификация подписей квантовых волн
    /// При наличии фичи `zebra_batch` использует ed25519-zebra с реальным батч-проверяющим,
    /// иначе выполняет параллельную проверку через ed25519-dalek + rayon.
    pub fn verify_waves_batch<'a>(&self, items: &[( &'a QuantumWave, VerifyingKey )]) -> bool {
        #[cfg(feature = "zebra_batch")]
        {
            let mut verifier = ZebraVerifier::new();
            for (wave, dalek_vk) in items.iter() {
                // Сообщение волны
                let msg = format!("{}:{}:{}:{}", wave.id, wave.amplitude, wave.phase, wave.shard_id);
                // Преобразуем ключ и подпись в типы zebra
                let vk_bytes = dalek_vk.to_bytes(); // [u8; 32]
                let zvk = ZebraVKBytes::from(vk_bytes);
                if wave.signature.len() != 64 { return false; }
                let mut sig_bytes = [0u8; 64];
                sig_bytes.copy_from_slice(&wave.signature);
                let zsig = match ZebraSig::try_from(sig_bytes) {
                    Ok(s) => s,
                    Err(_) => return false,
                };
                // queue принимает Item, который реализует From<(VerificationKeyBytes, Signature, &M)>
                verifier.queue((zvk, zsig, msg.as_bytes()));
            }
            let mut rng = OsRng;
            verifier.verify(&mut rng).is_ok()
        }
        #[cfg(not(feature = "zebra_batch"))]
        {
            self.verify_waves_parallel(items)
        }
    }

    /// Рассчитывает СУММАРНУЮ интерференцию всех волн (не парную!)
    pub fn calculate_total_interference(&self, resolution: usize) -> Vec<InterferencePoint> {
        let step = 1.0 / resolution as f64;
        
        // Нормализуем амплитуду относительно базового разрешения (100 точек)
        let normalization_factor = resolution as f64 / 100.0;
        
        (0..resolution).into_par_iter().map(|i| {
            let x = i as f64 * step;
            let mut total_amplitude = 0.0;
            let mut total_phase = 0.0;
            
            // Суммируем вклад ВСЕХ волн в точке x
            for wave in self.active_waves.values() {
                // Правильная физика: 2π * x + phase
                let contribution = wave.amplitude * (2.0 * std::f64::consts::PI * x + wave.phase).cos();
                total_amplitude += contribution;
                
                // Фаза как средневзвешенная
                total_phase += wave.phase * wave.amplitude;
            }
            
            // Нормализуем амплитуду для высокого разрешения
            let normalized_amplitude = total_amplitude * normalization_factor;
            
            // Нормализуем фазу
            let total_amplitude_sum: f64 = self.active_waves.values().map(|w| w.amplitude).sum();
            let normalized_phase = if total_amplitude_sum > 0.0 {
                total_phase / total_amplitude_sum
            } else {
                0.0
            };
            
            InterferencePoint {
                position: x,
                amplitude: normalized_amplitude, // Нормализованная амплитуда!
                phase: normalized_phase,
            }
        }).collect()
    }

    /// Обновляет интерференционный паттерн с высоким разрешением
    pub fn update_interference(&mut self, resolution: usize) {
        let cache_key = self.generate_cache_key();
        
        // Проверяем кэш
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(&cache_key) {
                self.interference_pattern = cached.clone();
                return;
            }
        }
        
        // Рассчитываем СУММАРНУЮ интерференцию всех волн
        self.interference_pattern = self.calculate_total_interference(resolution);
        
        // Кэшируем результат
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(cache_key, self.interference_pattern.clone());
        }
    }

    /// Рассчитывает интерференцию с высоким разрешением (10k+ точек)
    pub fn calculate_interference_pattern(&self, resolution: Option<usize>) -> Vec<InterferencePoint> {
        let resolution = resolution.unwrap_or(10_000); // Высокое разрешение по умолчанию
        
        if self.active_waves.is_empty() {
            return Vec::new();
        }
        
        self.calculate_total_interference(resolution)
    }

    fn generate_cache_key(&self) -> String {
        let mut keys: Vec<String> = self.active_waves.keys().cloned().collect();
        keys.sort();
        keys.join("|")
    }

    pub fn measure(&self, key: &str) -> Option<StateVector> {
        if let Ok(wave) = self.get_wave(key) {
            // Коллапсируем суперпозицию при измерении
            if let Some(state_vector) = wave.superposition.first() {
                return Some(state_vector.clone());
            }
        }
        None
    }

    pub fn get_active_wave_count(&self) -> usize {
        self.active_waves.len()
    }

    pub fn get_wave_lifetimes(&self) -> Vec<Duration> {
        let now = Instant::now();
        self.active_waves.values()
            .map(|wave| now.duration_since(wave.created_at))
            .collect()
    }

    /// Удаляет полностью погашенные волны
    pub fn remove_extinguished_waves(&mut self) {
        self.active_waves.retain(|_, wave| {
            wave.amplitude > 0.01 && // Амплитуда выше порога
            Instant::now().duration_since(wave.created_at) < wave.lifetime
        });
    }

    /// Обновляет амплитуды волн на основе результатов валидации
    pub fn update_wave_amplitudes(&mut self, validation_results: &HashMap<String, WaveValidationResult>) {
        for (wave_id, result) in validation_results {
            if let Some(wave) = self.active_waves.get_mut(wave_id) {
                wave.update_amplitude(result);
            }
        }
    }

    /// Получает статистику по волнам
    pub fn get_wave_statistics(&self) -> WaveStatistics {
        let total_waves = self.active_waves.len();
        let total_amplitude: f64 = self.active_waves.values().map(|w| w.amplitude).sum();
        let avg_amplitude = if total_waves > 0 { total_amplitude / total_waves as f64 } else { 0.0 };
        
        let strong_waves = self.active_waves.values().filter(|w| w.amplitude > 0.5).count();
        let weak_waves = self.active_waves.values().filter(|w| w.amplitude < 0.1).count();
        
        WaveStatistics {
            total_waves,
            total_amplitude,
            average_amplitude: avg_amplitude,
            strong_waves,
            weak_waves,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WaveStatistics {
    pub total_waves: usize,
    pub total_amplitude: f64,
    pub average_amplitude: f64,
    pub strong_waves: usize,
    pub weak_waves: usize,
}

#[derive(Debug, Clone)]
pub struct WaveHealth {
    pub wave_id: String,
    pub amplitude: f64,
    pub age_ratio: f64,
    pub status: WaveHealthStatus,
}

#[derive(Debug, Clone)]
pub enum WaveHealthStatus {
    Young,    // Молодая волна
    Mature,   // Зрелая волна
    Aging,    // Стареющая волна
    Expired,  // Истекшая волна
}

impl QuantumWave {
    pub fn new(id: String, amplitude: f64, phase: f64, shard_id: String, superposition: Vec<StateVector>) -> Self {
        Self {
            id,
            amplitude,
            phase,
            shard_id,
            created_at: Instant::now(),
            lifetime: Duration::from_millis(100), // 100ms время жизни волны
            superposition,
            signature: vec![], // Инициализируем пустой подписью
        }
    }

    /// Создает подписанную квантовую волну
    pub fn new_signed(
        id: String, 
        amplitude: f64, 
        phase: f64, 
        shard_id: String, 
        superposition: Vec<StateVector>,
        secret_key: &SigningKey
    ) -> Self {
        let mut wave = Self::new(id.clone(), amplitude, phase, shard_id, superposition);
        
        // Подписываем САМУ ВОЛНУ, а не транзакцию!
        let wave_message = format!(
            "{}:{}:{}:{}",
            wave.id,
            wave.amplitude,
            wave.phase,
            wave.shard_id
        );

        let sig: Signature = secret_key.sign(wave_message.as_bytes());
        wave.signature = sig.to_bytes().to_vec();
        
        wave
    }
    
    /// Проверяет подпись волны
    pub fn verify_signature(&self, public_key: &VerifyingKey) -> bool {
        if self.signature.is_empty() {
            return false;
        }
        
        let wave_message = format!("{}:{}:{}:{}", 
            self.id, 
            self.amplitude, 
            self.phase, 
            self.shard_id
        );
        match Signature::from_slice(&self.signature) {
            Ok(sig) => public_key.verify(wave_message.as_bytes(), &sig).is_ok(),
            Err(_) => false,
        }
    }

    /// Обновляет амплитуду на основе результата валидации
    pub fn update_amplitude(&mut self, validation_result: &WaveValidationResult) {
        match validation_result {
            WaveValidationResult::Validated(_) => {
                // Усиливаем волну
                self.amplitude *= 1.5;
                self.lifetime = Duration::from_secs(600); // Увеличиваем время жизни
            },
            WaveValidationResult::PartiallyValidated(_) => {
                // Частично ослабляем
                self.amplitude *= 0.7;
                self.lifetime = Duration::from_secs(300);
            },
            WaveValidationResult::Rejected(_) => {
                // Гасим волну
                self.amplitude *= 0.1;
                self.lifetime = Duration::from_millis(50);
            }
        }
    }
}

impl Default for QuantumWave {
    fn default() -> Self {
        Self {
            id: String::new(),
            amplitude: 1.0,
            phase: 0.0,
            shard_id: String::new(),
            created_at: Instant::now(),
            lifetime: Duration::from_millis(100),
            superposition: vec![],
            signature: vec![],
        }
    }
}

// Удаляем старую структуру QuantumState для обратной совместимости
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumState {
    pub id: String,
    pub amplitude: f64,
    pub phase: f64,
    pub shard_id: String,
    pub superposition: Vec<StateVector>,
}

impl QuantumState {
    pub fn new(id: String, amplitude: f64, phase: f64, shard_id: String, superposition: Vec<StateVector>) -> Self {
        Self {
            id,
            amplitude,
            phase,
            shard_id,
            superposition,
        }
    }
}

impl Default for QuantumState {
    fn default() -> Self {
        Self {
            id: String::new(),
            amplitude: 1.0,
            phase: 0.0,
            shard_id: String::new(),
            superposition: vec![],
        }
    }
}

// Конвертер для обратной совместимости
impl From<QuantumWave> for QuantumState {
    fn from(wave: QuantumWave) -> Self {
        Self {
            id: wave.id,
            amplitude: wave.amplitude,
            phase: wave.phase,
            shard_id: wave.shard_id,
            superposition: wave.superposition,
        }
    }
}

// Безаллокативное преобразование из ссылки на волну — избегаем clone() на больших структурах
impl From<&QuantumWave> for QuantumState {
    fn from(wave: &QuantumWave) -> Self {
        Self {
            id: wave.id.clone(),
            amplitude: wave.amplitude,
            phase: wave.phase,
            shard_id: wave.shard_id.clone(),
            superposition: wave.superposition.clone(),
        }
    }
}

// Конвертер из QuantumState в QuantumWave
impl From<QuantumState> for QuantumWave {
    fn from(state: QuantumState) -> Self {
        Self {
            id: state.id,
            amplitude: state.amplitude,
            phase: state.phase,
            shard_id: state.shard_id,
            created_at: Instant::now(),
            lifetime: Duration::from_secs(300), // 5 минут по умолчанию
            superposition: state.superposition,
            signature: vec![], // Инициализируем пустой подписью
        }
    }
}

#[derive(Debug, Clone)]
pub struct InterferenceField {
    pub cache: Arc<RwLock<HashMap<String, Vec<InterferencePoint>>>>,
} 

// Добавляем enum для результатов валидации волны
#[derive(Debug, Clone)]
pub enum WaveValidationResult {
    Validated(f64),           // Волна валидирована с амплитудой
    PartiallyValidated(f64),  // Частично валидирована
    Rejected(String),         // Отторгнута
}

#[derive(Debug, Clone)]
pub enum WaveFate {
    Amplify,    // Усилить волну
    Attenuate,  // Ослабить волну
    Cancel,     // Погасить волну
} 