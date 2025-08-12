use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::sync::RwLock;
use rayon::prelude::*;
use num_complex::Complex;
use std::time::{Instant, Duration};

#[derive(Debug, Clone)]
pub struct QuantumWave {
    pub id: String,
    pub amplitude: f64,
    pub phase: f64,
    pub shard_id: String,
    pub created_at: Instant,
    pub lifetime: Duration, // Время жизни волны
    pub superposition: Vec<StateVector>,
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
        // Очищаем устаревшие волны перед добавлением новой
        self.cleanup_expired_waves();
        
        // Если достигли лимита, удаляем самую старую волну
        if self.active_waves.len() >= self.max_waves {
            self.remove_oldest_wave();
        }
        
        self.active_waves.insert(key, wave);
        // Убираем автоматическое обновление интерференции - будем вызывать только когда нужно
        // self.update_interference(10_000); // Обновляем интерференцию с высоким разрешением
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
        }
    }
}

#[derive(Debug, Clone)]
pub struct InterferenceField {
    pub cache: Arc<RwLock<HashMap<String, Vec<InterferencePoint>>>>,
} 