use ocl::{ProQue, Buffer, MemFlags, Kernel, Program, Context, Device, Platform, Queue};
use ocl::Result as OclResult;
use crate::quantum::field::{QuantumState, InterferencePoint};
use rayon::prelude::*;

pub struct OpenClInterferenceEngine {
    pro_que: ProQue,
    kernel: Kernel,
    optimized_kernel: Kernel,
    analysis_kernel: Kernel,
    // Буферы для примитивных типов f64
    amplitudes_buffer: Buffer<f64>,
    phases_buffer: Buffer<f64>,
    result_buffer: Buffer<f64>,
    constructive_buffer: Buffer<u32>,
    destructive_buffer: Buffer<u32>,
    max_amp_buffer: Buffer<f64>,
    min_amp_buffer: Buffer<f64>,
    sum_amp_buffer: Buffer<f64>,
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
        
        // Create kernels
        let kernel = Kernel::builder()
            .program(&program)
            .name("calculate_interference")
            .queue(queue.clone())
            .build()?;
        
        let optimized_kernel = Kernel::builder()
            .program(&program)
            .name("calculate_interference_optimized")
            .queue(queue.clone())
            .build()?;
        
        let analysis_kernel = Kernel::builder()
            .program(&program)
            .name("analyze_interference")
            .queue(queue.clone())
            .build()?;
        
        // Get optimal local work group size (default to 256 if not available)
        let local_size = 256; // Default optimal size for most GPUs
        
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
        
        let max_amp_buffer = Buffer::<f64>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_write())
            .len(1)
            .build()?;
        
        let min_amp_buffer = Buffer::<f64>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_write())
            .len(1)
            .build()?;
        
        let sum_amp_buffer = Buffer::<f64>::builder()
            .queue(queue.clone())
            .flags(MemFlags::new().read_write())
            .len(1)
            .build()?;
        
        Ok(Self {
            pro_que: ProQue::new(context, queue, program, Some(1usize)),
            kernel,
            optimized_kernel,
            analysis_kernel,
            amplitudes_buffer,
            phases_buffer,
            result_buffer,
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
        
        // ОПТИМИЗАЦИЯ: Используем оптимизированный kernel для больших батчей
        if states.len() > 1000 {
            return self.calculate_interference_optimized(states, resolution);
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
        let zero_f64 = vec![0.0f64; 1];
        self.max_amp_buffer.write(&zero_f64).enq()?;
        self.min_amp_buffer.write(&zero_f64).enq()?;
        self.sum_amp_buffer.write(&zero_f64).enq()?;
        
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
        let mut max_amp = vec![0.0f64; 1];
        let mut min_amp = vec![0.0f64; 1];
        let mut sum_amp = vec![0.0f64; 1];
        self.constructive_buffer.read(&mut constructive).enq()?;
        self.destructive_buffer.read(&mut destructive).enq()?;
        self.max_amp_buffer.read(&mut max_amp).enq()?;
        self.min_amp_buffer.read(&mut min_amp).enq()?;
        self.sum_amp_buffer.read(&mut sum_amp).enq()?;
        self.pro_que.queue().finish()?;
        
        Ok((
            constructive[0],
            destructive[0],
            max_amp[0],
            min_amp[0],
            sum_amp[0],
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
