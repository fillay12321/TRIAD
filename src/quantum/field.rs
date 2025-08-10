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
            max_waves: 10, // Ограничиваем количество активных волн
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
        self.update_interference();
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

    pub fn update_interference(&mut self) {
        // Проверяем кэш
        let cache_key = self.generate_cache_key();
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(&cache_key) {
                self.interference_pattern = cached.clone();
                return;
            }
        }

        self.interference_pattern.clear();
        
        // Вычисляем интерференцию только между активными волнами
        let waves: Vec<&QuantumWave> = self.active_waves.values().collect();
        if waves.len() < 2 {
            return; // Нужно минимум 2 волны для интерференции
        }
        
        // Параллельное вычисление интерференции
        let interference_points: Vec<Vec<InterferencePoint>> = waves.par_iter()
            .enumerate()
            .flat_map(|(i, wave1)| {
                waves[i+1..].iter()
                    .map(|wave2| self.calculate_interference_fast(wave1, wave2))
                    .collect::<Vec<_>>()
            })
            .collect();
        
        // Объединяем все точки интерференции
        self.interference_pattern = interference_points.into_iter()
            .flatten()
            .collect();

        // Сохраняем в кэш
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(cache_key, self.interference_pattern.clone());
        }
    }

    fn calculate_interference_fast(&self, wave1: &QuantumWave, wave2: &QuantumWave) -> Vec<InterferencePoint> {
        let mut points = Vec::with_capacity(100);
        let step = 1.0 / 100.0;
        
        for i in 0..100 {
            let x = i as f64 * step;
            let amplitude1 = wave1.amplitude * (wave1.phase + x).cos();
            let amplitude2 = wave2.amplitude * (wave2.phase + x).cos();
            
            let interference_amplitude = amplitude1 + amplitude2;
            let interference_phase = (amplitude1 + amplitude2).atan2(amplitude1 - amplitude2);
            
            points.push(InterferencePoint {
                position: x,
                amplitude: interference_amplitude.abs(),
                phase: interference_phase,
            });
        }
        
        points
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

#[derive(Debug, Clone)]
pub struct InterferenceField {
    pub cache: Arc<RwLock<HashMap<String, Vec<InterferencePoint>>>>,
} 