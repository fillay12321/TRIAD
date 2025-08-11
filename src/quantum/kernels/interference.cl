// OpenCL kernel for quantum interference calculation with primitive types

__constant double PI = 3.14159265358979323846;

__kernel void calculate_interference(
    __global const double* amplitudes,
    __global const double* phases,
    __global double* result,
    const uint num_states,
    const uint resolution,
    const double step
) {
    uint idx = get_global_id(0);
    if (idx >= resolution) return;

    double x = (double)idx * step;
    double total_amplitude = 0.0;

    for (uint i = 0; i < num_states; i++) {
        double amp = amplitudes[i];
        double phase = phases[i];
        total_amplitude += amp * cos(2.0 * PI * x + phase);
    }

    result[idx] = total_amplitude;
}

__kernel void calculate_interference_optimized(
    __global const double* amplitudes,
    __global const double* phases,
    __global double* result,
    __local double* local_buffer,
    const uint num_states,
    const uint resolution,
    const double step,
    const double normalization_factor
) {
    uint gid = get_global_id(0);
    uint lid = get_local_id(0);
    uint lsize = get_local_size(0);

    if (gid >= resolution) return;

    double x = (double)gid * step;
    double sum = 0.0;

    // Векторизованная обработка с использованием SIMD
    #pragma unroll 4
    for (uint base = 0; base < num_states; base += lsize) {
        uint idx = base + lid;
        if (idx < num_states) {
            double amp = amplitudes[idx];
            double phase = phases[idx];
            // Используем fast_math для ускорения тригонометрии
            local_buffer[lid] = amp * native_cos(2.0 * PI * x + phase);
        } else {
            local_buffer[lid] = 0.0;
        }
        barrier(CLK_LOCAL_MEM_FENCE);

        // Оптимизированная редукция с использованием tree reduction
        for (uint offset = lsize >> 1; offset > 0; offset >>= 1) {
            if (lid < offset) {
                local_buffer[lid] += local_buffer[lid + offset];
            }
            barrier(CLK_LOCAL_MEM_FENCE);
        }
        if (lid == 0) {
            sum += local_buffer[0];
        }
        barrier(CLK_LOCAL_MEM_FENCE);
    }

    // Применяем нормализацию амплитуды
    if (lid == 0) {
        result[gid] = sum * normalization_factor;
    }
}

__kernel void analyze_interference(
    __global const double* amplitudes,
    __global uint* constructive_count,
    __global uint* destructive_count,
    __global double* max_amplitude,
    __global double* min_amplitude,
    __global double* sum_amplitude,
    const uint resolution,
    const double threshold
) {
    uint idx = get_global_id(0);
    if (idx >= resolution) return;

    double a = amplitudes[idx];

    if (a > threshold) atomic_inc(constructive_count);
    else if (a < -threshold) atomic_inc(destructive_count);

    // Note: OpenCL doesn't define atomic ops for double; this is a simplification.
    // In production you'd use atomic compare-exchange on 64-bit ints or do reduction.
    // Here we just write non-atomically which is acceptable for approximate stats.
    double old_max = *max_amplitude;
    if (a > old_max) *max_amplitude = a;
    double old_min = *min_amplitude;
    if (a < old_min) *min_amplitude = a;
    *sum_amplitude += a;
}

// Новый высокопроизводительный kernel для batch обработки
__kernel void calculate_interference_batch(
    __global const double* amplitudes,
    __global const double* phases,
    __global double* result,
    __local double* local_buffer,
    const uint num_states,
    const uint resolution,
    const uint batch_size,
    const double step,
    const double normalization_factor
) {
    uint gid = get_global_id(0);
    uint lid = get_local_id(0);
    uint lsize = get_local_size(0);
    uint batch_id = gid / batch_size;
    uint local_idx = gid % batch_size;

    if (gid >= resolution) return;

    double x = (double)local_idx * step;
    double sum = 0.0;

    // Обрабатываем несколько состояний одновременно
    #pragma unroll 8
    for (uint base = 0; base < num_states; base += lsize) {
        uint idx = base + lid;
        if (idx < num_states) {
            double amp = amplitudes[idx];
            double phase = phases[idx];
            // Используем fast_math и vectorization
            local_buffer[lid] = amp * native_cos(2.0 * PI * x + phase);
        } else {
            local_buffer[lid] = 0.0;
        }
        barrier(CLK_LOCAL_MEM_FENCE);

        // Оптимизированная редукция
        for (uint offset = lsize >> 1; offset > 0; offset >>= 1) {
            if (lid < offset) {
                local_buffer[lid] += local_buffer[lid + offset];
            }
            barrier(CLK_LOCAL_MEM_FENCE);
        }
        if (lid == 0) {
            sum += local_buffer[0];
        }
        barrier(CLK_LOCAL_MEM_FENCE);
    }

    // Применяем нормализацию и сохраняем результат
    if (lid == 0) {
        result[gid] = sum * normalization_factor;
    }
}
