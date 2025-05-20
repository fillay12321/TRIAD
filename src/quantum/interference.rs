use serde::{Serialize, Deserialize};
use serde::ser::SerializeStruct;
use serde::de::{Deserializer, Visitor};
use std::fmt;
use crate::quantum::field::{QuantumState, InterferencePoint};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct InterferenceEngine {
    pub resolution: usize,
    pub threshold: f64,
    pub decay_factor: f64,
    cache: Arc<RwLock<HashMap<String, Vec<InterferencePoint>>>>,
}

impl InterferenceEngine {
    pub fn new(resolution: usize, decay_factor: f64) -> Self {
        Self {
            resolution: resolution.min(100), // Ограничиваем разрешение
            threshold: 0.95,
            decay_factor,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn calculate_interference_pattern(&self, states: &[QuantumState]) -> Vec<InterferencePoint> {
        // Проверяем кэш
        let cache_key = self.generate_cache_key(states);
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
        }

        let step = 1.0 / self.resolution as f64;
        
        // Предварительно вычисляем фазы и амплитуды
        let phases: Vec<f64> = states.iter()
            .map(|state| state.phase)
            .collect();
        
        let amplitudes: Vec<f64> = states.iter()
            .map(|state| state.amplitude)
            .collect();

        // Параллельное вычисление точек интерференции
        let pattern: Vec<InterferencePoint> = (0..self.resolution)
            .into_par_iter()
            .map(|i| {
                let x = i as f64 * step;
                let mut total_amplitude = 0.0;
                let mut total_phase = 0.0;
                
                // Используем предварительно вычисленные значения
                for (amplitude, phase) in amplitudes.iter().zip(phases.iter()) {
                    let contribution = self.calculate_state_contribution_fast(*amplitude, *phase, x);
                    total_amplitude += contribution.0;
                    total_phase += contribution.1;
                }
                
                // Применяем затухание (оптимизированная версия)
                total_amplitude *= self.decay_factor.powi((x * 10.0) as i32);
                
                InterferencePoint {
                    position: x,
                    amplitude: total_amplitude,
                    phase: total_phase,
                }
            })
            .collect();
        
        // Сохраняем в кэш
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(cache_key, pattern.clone());
        }
        
        pattern
    }
    
    fn calculate_state_contribution_fast(&self, amplitude: f64, phase: f64, x: f64) -> (f64, f64) {
        // Оптимизированная версия с предварительными вычислениями
        let pi_2 = 2.0 * std::f64::consts::PI;
        let phase_x = pi_2 * x + phase;
        let wave = amplitude * phase_x.cos();
        (wave, phase)
    }

    fn generate_cache_key(&self, states: &[QuantumState]) -> String {
        // Оптимизированное создание ключа кэша
        let mut key = String::with_capacity(states.len() * 20);
        for state in states {
            key.push_str(&format!("{:.3}_{:.3}_", state.amplitude, state.phase));
        }
        key
    }
    
    pub fn analyze_interference(&self, pattern: &[InterferencePoint]) -> InterferenceAnalysis {
        let mut analysis = InterferenceAnalysis {
            max_amplitude: 0.0,
            min_amplitude: f64::MAX,
            average_amplitude: 0.0,
            constructive_points: Vec::with_capacity(pattern.len() / 2),
            destructive_points: Vec::with_capacity(pattern.len() / 2),
        };
        
        // Параллельный анализ
        let (max_amp, min_amp, sum_amp, constructive, destructive) = pattern.par_iter()
            .fold(
                || (0.0f64, f64::MAX, 0.0f64, Vec::new(), Vec::new()),
                |(max, min, sum, mut constr, mut destr), point| {
                    let amplitude = point.amplitude;
                    let new_max = max.max(amplitude);
                    let new_min = min.min(amplitude);
                    let new_sum = sum + amplitude;
                    
                    if amplitude > 0.5 {
                        constr.push(point.clone());
                    } else if amplitude < -0.5 {
                        destr.push(point.clone());
                    }
                    
                    (new_max, new_min, new_sum, constr, destr)
                }
            )
            .reduce(
                || (0.0f64, f64::MAX, 0.0f64, Vec::new(), Vec::new()),
                |(max1, min1, sum1, mut constr1, mut destr1),
                 (max2, min2, sum2, mut constr2, mut destr2)| {
                    constr1.extend(constr2);
                    destr1.extend(destr2);
                    (max1.max(max2), min1.min(min2), sum1 + sum2, constr1, destr1)
                }
            );
        
        analysis.max_amplitude = max_amp;
        analysis.min_amplitude = min_amp;
        analysis.average_amplitude = sum_amp / pattern.len() as f64;
        analysis.constructive_points = constructive;
        analysis.destructive_points = destructive;
        
        analysis
    }

    pub fn export_pattern_to_csv(&self, pattern: &[InterferencePoint], path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;
        let mut file = File::create(path)?;
        writeln!(file, "position,amplitude,phase")?;
        for point in pattern {
            writeln!(file, "{},{},{}", point.position, point.amplitude, point.phase)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct InterferenceAnalysis {
    pub max_amplitude: f64,
    pub min_amplitude: f64,
    pub average_amplitude: f64,
    pub constructive_points: Vec<InterferencePoint>,
    pub destructive_points: Vec<InterferencePoint>,
}

#[derive(Debug, Clone)]
pub struct InterferencePattern {
    pub cache: Arc<RwLock<HashMap<String, Vec<InterferencePoint>>>>,
} 