
use crate::quantum::field::{QuantumState, InterferencePoint, QuantumField};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use rayon::prelude::*;
use sha2::{Sha256, Digest};
use serde_json;

#[cfg(feature = "cuda")]
use cust::prelude::*;
#[cfg(feature = "opencl")]
use ocl::{ProQue, Buffer};

pub trait QuantumInterferenceAccelerator {
    fn calculate(&self, states: &[QuantumState], resolution: usize) -> Vec<InterferencePoint>;
}

pub struct CpuAccelerator;
impl QuantumInterferenceAccelerator for CpuAccelerator {
    fn calculate(&self, states: &[QuantumState], resolution: usize) -> Vec<InterferencePoint> {
        let step = 1.0 / resolution as f64;
        (0..resolution).into_par_iter().map(|i| {
            let x = i as f64 * step;
            let mut total_amp = 0.0;
            for s in states {
                total_amp += s.amplitude * (2.0 * std::f64::consts::PI * x + s.phase).cos();
            }
            InterferencePoint { position: x, amplitude: total_amp, phase: 0.0 }
        }).collect()
    }
}

#[cfg(feature = "cuda")]
pub struct CudaAccelerator {
    _ctx: cust::context::Context,
}
#[cfg(feature = "cuda")]
impl QuantumInterferenceAccelerator for CudaAccelerator {
    fn calculate(&self, states: &[QuantumState], resolution: usize) -> Vec<InterferencePoint> {
        // CUDA kernel launch (stub, kernel implementation required)
        vec![InterferencePoint { position: 0.0, amplitude: 0.0, phase: 0.0 }; resolution]
    }
}

#[cfg(feature = "opencl")]
pub struct OpenClAccelerator {
    engine: crate::quantum::opencl::OpenClInterferenceEngine,
}

#[cfg(feature = "opencl")]
impl OpenClAccelerator {
    pub fn new() -> Result<Self, String> {
        match crate::quantum::opencl::OpenClInterferenceEngine::new() {
            Ok(engine) => Ok(Self { engine }),
            Err(e) => Err(format!("Failed to initialize OpenCL engine: {}", e)),
        }
    }
}

#[cfg(feature = "opencl")]
impl QuantumInterferenceAccelerator for OpenClAccelerator {
    fn calculate(&self, states: &[QuantumState], resolution: usize) -> Vec<InterferencePoint> {
        match self.engine.calculate_interference(states, resolution) {
            Ok(points) => points,
            Err(e) => {
                eprintln!("OpenCL calculation failed: {}, falling back to CPU", e);
                // Fallback to CPU
                let cpu_acc = CpuAccelerator;
                cpu_acc.calculate(states, resolution)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct InterferenceEngine {
    pub resolution: usize,
    pub threshold: f64,
    pub decay_factor: f64,
    cache: Arc<RwLock<HashMap<String, Vec<InterferencePoint>>>>,
    // Reusable buffer to avoid per-call allocations
    state_buffer: Vec<QuantumState>,
}

impl InterferenceEngine {
    pub fn new(resolution: usize, decay_factor: f64) -> Self {
        Self {
            // Разрешение до 10_000 точек (соответствует pitch "High-resolution patterns")
            resolution: resolution.min(10_000),
            threshold: 0.95,
            decay_factor,
            cache: Arc::new(RwLock::new(HashMap::new())),
            state_buffer: Vec::new(),
        }
    }

    pub fn calculate_interference_pattern(&self, states: &[QuantumState]) -> Vec<InterferencePoint> {
        // Check cache
        let cache_key = self.generate_cache_key(states);
        if let Ok(cache) = self.cache.read() {
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
        }

        let step = 1.0 / self.resolution as f64;
        
        // Parallel interference point calculation
        let pattern: Vec<InterferencePoint> = (0..self.resolution)
            .into_par_iter()
            .map(|i| {
                let x = i as f64 * step;
                let mut total_amplitude = 0.0;
                let mut total_phase = 0.0;
                
                // Прямой проход по состояниям без промежуточных векторов
                for state in states {
                    let contribution = self.calculate_state_contribution_fast(state.amplitude, state.phase, x);
                    total_amplitude += contribution.0;
                    total_phase += contribution.1;
                }
                
                // Экспоненциальное затухание (стабильно и быстро)
                total_amplitude *= (-x * self.decay_factor).exp();
                
                InterferencePoint {
                    position: x,
                    amplitude: total_amplitude,
                    phase: total_phase,
                }
            })
            .collect();
        
        // Save to cache
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(cache_key, pattern.clone());
        }
        
        pattern
    }

    /// Fills an internal buffer from the given QuantumField and runs one interference calculation
    /// Reuses the buffer to avoid O(n) allocations on every call
    pub fn calculate_interference_from_field(&mut self, field: &QuantumField) -> Vec<InterferencePoint> {
        // Prepare buffer
        self.state_buffer.clear();
        self.state_buffer.reserve(field.active_waves.len());
        // Map waves to states, избегая избыточного clone() на самой волне
        for wave in field.active_waves.values() {
            self.state_buffer.push(QuantumState::from(wave));
        }
        // Run calculation with GPU acceleration if available
        self.calculate_interference_auto(&self.state_buffer)
    }
    
    fn calculate_state_contribution_fast(&self, amplitude: f64, phase: f64, x: f64) -> (f64, f64) {
        // Optimized version with pre-calculated values
        let pi_2 = 2.0 * std::f64::consts::PI;
        let phase_x = pi_2 * x + phase;
        let wave = amplitude * phase_x.cos();
        (wave, phase)
    }

    fn generate_cache_key(&self, states: &[QuantumState]) -> String {
        // New protected key: serialization + SHA256
        let json = serde_json::to_string(states).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        format!("{:x}", hasher.finalize())
    }
    
    pub fn analyze_interference(&self, pattern: &[InterferencePoint]) -> InterferenceAnalysis {
        let mut analysis = InterferenceAnalysis {
            max_amplitude: 0.0,
            min_amplitude: f64::MAX,
            average_amplitude: 0.0,
            constructive_points: Vec::with_capacity(pattern.len() / 2),
            destructive_points: Vec::with_capacity(pattern.len() / 2),
        };
        
        // Parallel analysis
        let (max_amp, min_amp, sum_amp, constructive, destructive) = pattern.par_iter()
            .fold(
                || (0.0f64, f64::MAX, 0.0f64, Vec::new(), Vec::new()),
                |(max, min, sum, mut constr, mut destr), point| {
                    let amplitude = point.amplitude;
                    let new_max = max.max(amplitude);
                    let new_min = min.min(amplitude);
                    let new_sum = sum + amplitude;
                    
                    if amplitude > 0.01 {
                        constr.push(point.clone());
                    } else if amplitude < -0.01 {
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

    pub fn calculate_interference_auto(&self, states: &[QuantumState]) -> Vec<InterferencePoint> {
        #[cfg(feature = "cuda")]
        {
            let ctx = cust::quick_init().unwrap();
            let acc = CudaAccelerator { _ctx: ctx }; // Kernel implementation required
            return acc.calculate(states, self.resolution);
        }
        #[cfg(all(not(feature = "cuda"), feature = "opencl"))]
        {
            match OpenClAccelerator::new() {
                Ok(acc) => {
                    println!("🚀 Using OpenCL acceleration");
                    return acc.calculate(states, self.resolution);
                }
                Err(e) => {
                    println!("⚠️  OpenCL initialization failed: {}, falling back to CPU", e);
                }
            }
        }
        // Fallback on CPU
        println!("💻 Using CPU fallback");
        let acc = CpuAccelerator;
        acc.calculate(states, self.resolution)
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

impl InterferenceAnalysis {
    /// Возвращает общее количество точек в паттерне
    pub fn total_points(&self) -> usize {
        self.constructive_points.len() + self.destructive_points.len()
    }
}

#[derive(Debug, Clone)]
pub struct InterferencePattern {
    pub cache: Arc<RwLock<HashMap<String, Vec<InterferencePoint>>>>,
} 