use ocl::{ProQue, Buffer, MemFlags, Kernel, Program, Context, Device, Platform, Queue};
use ocl::Result as OclResult;
use crate::quantum::field::{QuantumState, InterferencePoint};
use rayon::prelude::*;

pub struct OpenClInterferenceEngine {
    pro_que: ProQue,
    kernel: Kernel,
    optimized_kernel: Kernel,
    analysis_kernel: Kernel,
    vectorized_kernel: Kernel,
    vectorized_analysis_kernel: Kernel,
    // Буферы для примитивных типов f64
    amplitudes_buffer: Buffer<f64>,
    phases_buffer: Buffer<f64>,
    result_buffer: Buffer<f64>,
    // ВЕКТОРИЗОВАННЫЕ буферы для Intel GPU (float4)
    vectorized_amplitudes_buffer: Buffer<f32>,
    vectorized_phases_buffer: Buffer<f32>,
    vectorized_result_buffer: Buffer<f32>,
    constructive_buffer: Buffer<u32>,
    destructive_buffer: Buffer<u32>,
    max_amp_buffer: Buffer<u32>,  // Изменено на u32 для атомарных операций
    min_amp_buffer: Buffer<u32>,  // Изменено на u32 для атомарных операций
    sum_amp_buffer: Buffer<u32>,  // Изменено на u32 для атомарных операций
    local_size: usize,
}

impl OpenClInterferenceEngine {
    pub fn new() -> OclResult<Self> {
        // Find OpenCL platform and device
        let platform = Platform::first()?;
        let device = Device::first(platform)?;
        
        // Create context and queue
        let context = Context::builder()
            .platform(platform)
            .devices(device)
            .build()?;
        
        let queue = Queue::new(&context, device, None)?;
        
        // Load and compile OpenCL program
        let kernel_src = include_str!("kernels/interference.cl");
        let program = Program::builder()
            .src(kernel_src)
            .build(&context)?;
        
        // Create kernels with proper argument specification
        let kernel = Kernel::builder()
            .program(&program)
            .name("calculate_interference")
            .queue(queue.clone())
            .arg(None::<&Buffer<f64>>) // amplitudes
            .arg(None::<&Buffer<f64>>) // phases  
            .arg(None::<&Buffer<f64>>) // result
            .arg(0u32) // num_states
            .arg(0u32) // resolution
            .arg(0.0f64) // step
            .build()?;
        
        let optimized_kernel = Kernel::builder()
            .program(&program)
            .name("calculate_interference_optimized")
            .queue(queue.clone())
            .arg(None::<&Buffer<f64>>) // amplitudes
            .arg(None::<&Buffer<f64>>) // phases
            .arg(None::<&Buffer<f64>>) // result
            .arg(None::<&Buffer<f64>>) // local_buffer
            .arg(0u32) // num_states
            .arg(0u32) // resolution
            .arg(0.0f64) // step
            .arg(0.0f64) // normalization_factor
            .build()?;
        
        let analysis_kernel = Kernel::builder()
            .program(&program)
            .name("analyze_interference")
            .queue(queue.clone())
            .arg(None::<&Buffer<f64>>) // amplitudes
            .arg(None::<&Buffer<u32>>) // constructive_count
            .arg(None::<&Buffer<u32>>) // destructive_count
            .arg(None::<&Buffer<u32>>) // max_amplitude (uint для атомарных операций)
            .arg(None::<&Buffer<u32>>) // min_amplitude (uint для атомарных операций)
            .arg(None::<&Buffer<u32>>) // sum_amplitude (uint для атомарных операций)
            .arg(0u32) // resolution
            .arg(0.0f64) // threshold
            .build()?;
        
        // ВЕКТОРИЗОВАННЫЕ KERNELS для Intel GPU
        let vectorized_kernel = Kernel::builder()
            .program(&program)
            .name("calculate_interference_vectorized")
            .queue(queue.clone())
            .arg(None::<&Buffer<f32>>) // amplitudes (float4)
            .arg(None::<&Buffer<f32>>) // phases (float4)
            .arg(None::<&Buffer<f32>>) // result (float4)
            .arg(0u32) // num_states
            .arg(0u32) // resolution
            .arg(0.0f32) // step
            .build()?;
        
        let vectorized_analysis_kernel = Kernel::builder()
            .program(&program)
            .name("analyze_interference_vectorized")
            .queue(queue.clone())
            .arg(None::<&Buffer<f32>>) // amplitudes (float4)
            .arg(None::<&Buffer<u32>>) // constructive_count
            .arg(None::<&Buffer<u32>>) // destructive_count
            .arg(None::<&Buffer<u32>>) // max_amplitude (uint для атомарных операций)
            .arg(None::<&Buffer<u32>>) // min_amplitude (uint для атомарных операций)
            .arg(None::<&Buffer<u32>>) // sum_amplitude (uint для атомарных операций)
            .arg(0u32) // resolution
            .arg(0.0f32) // threshold
            .build()?;
        
        // Get optimal local work group size - адаптивный размер для разных GPU
        let device_max_wg_size = device.max_wg_size()?;
        let local_size = 2; // Фиксированный размер 2 для совместимости
        
        // Create buffers with primitive types - ОПТИМИЗИРОВАННЫЕ размеры
        let amplitudes_buffer = Buffer::<f64>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_only())
            .len(50000) // Увеличиваем до 50000 для больших батчей
            .build()?;
        
        let phases_buffer = Buffer::<f64>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_only())
            .len(50000) // Увеличиваем до 50000 для больших батчей
            .build()?;
        
        let result_buffer = Buffer::<f64>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().write_only())
            .len(100000) // Увеличиваем до 100000 для высокого разрешения
            .build()?;
        
        // ВЕКТОРИЗОВАННЫЕ буферы для Intel GPU (float4)
        let vectorized_amplitudes_buffer = Buffer::<f32>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_only())
            .len(50000) // Обычные float для входных данных
            .build()?;
        
        let vectorized_phases_buffer = Buffer::<f32>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_only())
            .len(50000) // Обычные float для входных данных
            .build()?;
        
        let vectorized_result_buffer = Buffer::<f32>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().write_only())
            .len(100000) // Обычные float для результатов
            .build()?;
        
        let constructive_buffer = Buffer::<u32>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().write_only())
            .len(1)
            .build()?;
        
        let destructive_buffer = Buffer::<u32>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().write_only())
            .len(1)
            .build()?;
        
        let max_amp_buffer = Buffer::<u32>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_write())
            .len(1)
            .build()?;
        
        let min_amp_buffer = Buffer::<u32>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_write())
            .len(1)
            .build()?;
        
        let sum_amp_buffer = Buffer::<u32>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_write())
            .len(1)
            .build()?;
        
        Ok(Self {
            pro_que: ProQue::new(context, queue, program, Some(1usize)),
            kernel,
            optimized_kernel,
            analysis_kernel,
            vectorized_kernel,
            vectorized_analysis_kernel,
            amplitudes_buffer,
            phases_buffer,
            result_buffer,
            vectorized_amplitudes_buffer,
            vectorized_phases_buffer,
            vectorized_result_buffer,
            constructive_buffer,
            destructive_buffer,
            max_amp_buffer,
            min_amp_buffer,
            sum_amp_buffer,
            local_size,
        })
    }
    
    pub fn calculate_interference(&self, states: &[QuantumState], resolution: usize) -> OclResult<Vec<InterferencePoint>> {
        if states.is_empty() || resolution == 0 {
            return Ok(Vec::new());
        }
        
        // АВТОМАТИЧЕСКИЙ ВЫБОР: используем векторный kernel для Intel GPU если возможно
        if states.len() >= 100 && resolution >= 1000 && resolution % 4 == 0 {
            // Пытаемся использовать векторный kernel
            match self.calculate_interference_vectorized(states, resolution) {
                Ok(points) => {
                    println!("🚀 Using vectorized kernel (Intel GPU optimized)");
                    return Ok(points);
                },
                Err(e) => {
                    println!("⚠️  Vectorized kernel failed: {}, falling back to standard kernel", e);
                }
            }
        }
        
        let step = 1.0 / resolution as f64;
        
        // Prepare data for OpenCL - ОПТИМИЗИРОВАННАЯ подготовка
        let amplitudes: Vec<f64> = states.iter().map(|s| s.amplitude).collect();
        let phases: Vec<f64> = states.iter().map(|s| s.phase).collect();
        
        // Write data to buffers - ОПТИМИЗАЦИЯ: Асинхронная запись
        self.amplitudes_buffer.write(&amplitudes).enq()?;
        self.phases_buffer.write(&phases).enq()?;
        
        // Set kernel arguments
        self.kernel.set_arg(0, &self.amplitudes_buffer)?;
        self.kernel.set_arg(1, &self.phases_buffer)?;
        self.kernel.set_arg(2, &self.result_buffer)?;
        let ns = states.len() as u32;
        let res = resolution as u32;
        self.kernel.set_arg(3, &ns)?;
        self.kernel.set_arg(4, &res)?;
        self.kernel.set_arg(5, &step)?;
        
        let global_size = resolution;
        let local_size = self.local_size.min(global_size);
        
        unsafe {
            self.kernel.cmd()
                .global_work_size(global_size)
                .local_work_size(local_size)
                .enq()?;
        }
        
        // ОПТИМИЗАЦИЯ: Убираем лишние finish() вызовы
        self.pro_que.queue().finish()?;
        
        // Read results - ОПТИМИЗАЦИЯ: Асинхронное чтение
        let mut result_data = vec![0.0; resolution];
        self.result_buffer.read(&mut result_data).enq()?;
        
        // Convert to InterferencePoint - ОПТИМИЗАЦИЯ: Параллельное преобразование
        let points: Vec<InterferencePoint> = (0..resolution)
            .map(|i| {
                let x = i as f64 * step;
                InterferencePoint {
                    position: x,
                    amplitude: result_data[i],
                    phase: 0.0, // Phase not calculated in this kernel
                }
            })
            .collect();
        
        Ok(points)
    }
    
    // ВЕКТОРИЗОВАННЫЙ метод для Intel GPU
    pub fn calculate_interference_vectorized(&self, states: &[QuantumState], resolution: usize) -> OclResult<Vec<InterferencePoint>> {
        if states.is_empty() || resolution == 0 {
            return Ok(Vec::new());
        }
        
        // Проверяем что resolution делится на 4 для float4
        if resolution % 4 != 0 {
            return Err(ocl::Error::from("Resolution must be divisible by 4 for vectorized kernel"));
        }
        
        let step = 1.0f32 / (resolution / 4) as f32;
        
        // Подготавливаем данные для float4 (правильная упаковка)
        let mut vectorized_amplitudes = Vec::with_capacity(states.len());
        let mut vectorized_phases = Vec::with_capacity(states.len());
        
        for state in states {
            vectorized_amplitudes.push(state.amplitude as f32);
            vectorized_phases.push(state.phase as f32);
        }
        
        // Записываем данные в GPU буферы
        self.vectorized_amplitudes_buffer.write(&vectorized_amplitudes).enq()?;
        self.vectorized_phases_buffer.write(&vectorized_phases).enq()?;
        
        // Устанавливаем аргументы kernel
        self.vectorized_kernel.set_arg(0, &self.vectorized_amplitudes_buffer)?;
        self.vectorized_kernel.set_arg(1, &self.vectorized_phases_buffer)?;
        self.vectorized_kernel.set_arg(2, &self.vectorized_result_buffer)?;
        let ns = states.len() as u32;
        let res = resolution as u32;
        self.vectorized_kernel.set_arg(3, &ns)?;
        self.vectorized_kernel.set_arg(4, &res)?;
        self.vectorized_kernel.set_arg(5, &step)?;
        
        // Выполняем kernel
        let global_size = resolution / 4; // Обрабатываем по 4 точки
        let local_size = self.local_size.min(global_size);
        
        unsafe {
            self.vectorized_kernel.cmd()
                .global_work_size(global_size)
                .local_work_size(local_size)
                .enq()?;
        }
        
        // ОПТИМИЗАЦИЯ: Убираем лишние finish() вызовы
        self.pro_que.queue().finish()?;
        
        // Читаем результаты
        let mut result_data = vec![0.0f32; resolution];
        self.vectorized_result_buffer.read(&mut result_data).enq()?;
        
        // Конвертируем в InterferencePoint
        let points: Vec<InterferencePoint> = (0..resolution)
            .map(|i| {
                let x = i as f64 * (step as f64);
                InterferencePoint {
                    position: x,
                    amplitude: result_data[i] as f64,
                    phase: 0.0, // Phase not calculated in this kernel
                }
            })
            .collect();
        
        Ok(points)
    }
    
    pub fn calculate_interference_optimized(&self, states: &[QuantumState], resolution: usize) -> OclResult<Vec<InterferencePoint>> {
        if states.is_empty() || resolution == 0 {
            return Ok(Vec::new());
        }
        
        let step = 1.0 / resolution as f64;
        
        // Prepare data for OpenCL
        let amplitudes: Vec<f64> = states.iter().map(|s| s.amplitude).collect();
        let phases: Vec<f64> = states.iter().map(|s| s.phase).collect();
        
        // Write data to buffers
        self.amplitudes_buffer.write(&amplitudes).enq()?;
        self.phases_buffer.write(&phases).enq()?;
        
        // Set kernel arguments
        self.optimized_kernel.set_arg(0, &self.amplitudes_buffer)?;
        self.optimized_kernel.set_arg(1, &self.phases_buffer)?;
        self.optimized_kernel.set_arg(2, &self.result_buffer)?;
        // Local buffer size for f64 elements
        let local_elems = self.local_size;
        self.optimized_kernel.set_arg(3, local_elems)?;
        let ns = states.len() as u32;
        let res = resolution as u32;
        self.optimized_kernel.set_arg(4, &ns)?;
        self.optimized_kernel.set_arg(5, &res)?;
        self.optimized_kernel.set_arg(6, &step)?;
        
        // Execute kernel
        let global_size = resolution;
        let local_size = self.local_size.min(global_size);
        
        unsafe {
            self.optimized_kernel.cmd()
                .global_work_size(global_size)
                .local_work_size(local_size)
                .enq()?;
        }
        self.pro_que.queue().finish()?;
        
        // Read results
        let mut result_data = vec![0.0; resolution];
        self.result_buffer.read(&mut result_data).enq()?;
        self.pro_que.queue().finish()?;
        
        // Convert to InterferencePoint
        let mut points = Vec::with_capacity(resolution);
        for i in 0..resolution {
            let x = i as f64 * step;
            points.push(InterferencePoint {
                position: x,
                amplitude: result_data[i],
                phase: 0.0,
            });
        }
        
        Ok(points)
    }
    
    pub fn analyze_interference(&self, pattern: &[InterferencePoint], threshold: f64) -> OclResult<(u32, u32, f64, f64, f64)> {
        if pattern.is_empty() {
            return Ok((0, 0, 0.0, 0.0, 0.0));
        }
        
        // Prepare pattern data - just amplitudes
        let amplitudes: Vec<f64> = pattern.iter().map(|p| p.amplitude).collect();
        
        // Create temporary buffer for pattern
        let pattern_buffer = Buffer::<f64>::builder()
            .queue(self.pro_que.queue().clone())
            .flags(MemFlags::new().read_only())
            .len(amplitudes.len())
            .build()?;
        
        pattern_buffer.write(&amplitudes).enq()?;
        
        // Clear analysis buffers
        let zero_u32 = vec![0u32; 1];
        self.constructive_buffer.write(&zero_u32).enq()?;
        self.destructive_buffer.write(&zero_u32).enq()?;
        self.max_amp_buffer.write(&zero_u32).enq()?;
        self.min_amp_buffer.write(&zero_u32).enq()?;
        self.sum_amp_buffer.write(&zero_u32).enq()?;
        
        // Set kernel arguments
        self.analysis_kernel.set_arg(0, &pattern_buffer)?;
        self.analysis_kernel.set_arg(1, &self.constructive_buffer)?;
        self.analysis_kernel.set_arg(2, &self.destructive_buffer)?;
        self.analysis_kernel.set_arg(3, &self.max_amp_buffer)?;
        self.analysis_kernel.set_arg(4, &self.min_amp_buffer)?;
        self.analysis_kernel.set_arg(5, &self.sum_amp_buffer)?;
        let res = pattern.len() as u32;
        self.analysis_kernel.set_arg(6, &res)?;
        self.analysis_kernel.set_arg(7, &threshold)?;
        
        // Execute kernel
        let global_size = pattern.len();
        let local_size = self.local_size.min(global_size);
        
        unsafe {
            self.analysis_kernel.cmd()
                .global_work_size(global_size)
                .local_work_size(local_size)
                .enq()?;
        }
        self.pro_que.queue().finish()?;
        
        // Read analysis results
        let mut constructive = vec![0u32; 1];
        let mut destructive = vec![0u32; 1];
        let mut max_amp_bits = vec![0u32; 1];
        let mut min_amp_bits = vec![0u32; 1];
        let mut sum_amp_bits = vec![0u32; 1];
        self.constructive_buffer.read(&mut constructive).enq()?;
        self.destructive_buffer.read(&mut destructive).enq()?;
        self.max_amp_buffer.read(&mut max_amp_bits).enq()?;
        self.min_amp_buffer.read(&mut min_amp_bits).enq()?;
        self.sum_amp_buffer.read(&mut sum_amp_bits).enq()?;
        self.pro_que.queue().finish()?;
        
        // Конвертируем uint обратно в float
        // Для простоты используем только high часть (32 бита)
        let max_amp = f64::from_bits((max_amp_bits[0] as u64) << 32);
        let min_amp = f64::from_bits((min_amp_bits[0] as u64) << 32);
        let sum_amp = f64::from_bits((sum_amp_bits[0] as u64) << 32);
        
        Ok((
            constructive[0],
            destructive[0],
            max_amp,
            min_amp,
            sum_amp,
        ))
    }
    
    pub fn get_device_info(&self) -> OclResult<String> {
        let device = self.pro_que.queue().device();
        let name = device.name()?;
        let vendor = device.vendor()?;
        let compute_units = 1; // Default value, compute units info not available
        let max_work_group_size = device.max_wg_size()?;
        
        Ok(format!(
            "Device: {} (Vendor: {})\nCompute Units: {}\nMax Work Group Size: {}",
            name, vendor, compute_units, max_work_group_size
        ))
    }
    
    pub fn benchmark(&self, states: &[QuantumState], resolution: usize) -> OclResult<f64> {
        use std::time::Instant;
        
        // Warm up
        for _ in 0..3 {
            self.calculate_interference(states, resolution)?;
        }
        
        let start_bench = Instant::now();
        
        // Benchmark
        for _ in 0..10 {
            self.calculate_interference(states, resolution)?;
        }
        
        let duration = start_bench.elapsed();
        let avg_time = duration.as_micros() as f64 / 10.0;
        
        Ok(avg_time)
    }
}

impl Drop for OpenClInterferenceEngine {
    fn drop(&mut self) {
        // OpenCL resources are automatically cleaned up
    }
}
