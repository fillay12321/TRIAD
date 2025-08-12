# 🚀 NVIDIA T4 Setup в Google Colab

## 📋 **Шаги для запуска в Google Colab:**

### 1️⃣ **Создайте новый notebook в Google Colab**
- Перейдите на [colab.research.google.com](https://colab.research.google.com)
- Создайте новый Python notebook

### 2️⃣ **Проверьте GPU**
```python
# Проверяем доступность GPU
!nvidia-smi
```

### 3️⃣ **Установите Rust**
```bash
# Устанавливаем Rust
!curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
!source $HOME/.cargo/env
!rustc --version
```

### 4️⃣ **Клонируйте репозиторий**
```bash
# Клонируем TRIAD
!git clone https://github.com/your-username/TRIAD.git
%cd TRIAD
```

### 5️⃣ **Установите зависимости**
```bash
# Устанавливаем системные зависимости
!apt-get update
!apt-get install -y build-essential libssl-dev pkg-config
```

### 6️⃣ **Скомпилируйте и запустите**
```bash
# Компилируем с GPU поддержкой
!cargo build --example quantum_consensus_demo --features "opencl,consensus,processor"

# Запускаем демо
!RUST_LOG=info cargo run --example quantum_consensus_demo --features "opencl,consensus,processor" -- --distributed --num_codespaces 1
```

## 🎯 **Ожидаемые результаты с NVIDIA T4:**

### 📊 **Производительность:**
- **Векторизация**: 4x ускорение (float4 операции)
- **Параллелизм**: 2560 CUDA ядер
- **Память**: 15 ГБ GDDR6
- **Скорость**: 10-50x быстрее CPU

### 🔍 **Логи должны показать:**
```
🚀 GPU acceleration available (OpenCL)
🚀 Using vectorized kernel (NVIDIA T4 optimized)
🔬 [Shard 0] Interference calculation: 5.234ms  # Было ~200ms на CPU
```

## ⚠️ **Возможные проблемы:**

### 1️⃣ **OpenCL не найден**
```bash
# Установите OpenCL
!apt-get install -y ocl-icd-opencl-dev
```

### 2️⃣ **Компиляция медленная**
```bash
# Используйте оптимизированную сборку
!cargo build --release --example quantum_consensus_demo --features "opencl,consensus,processor"
```

### 3️⃣ **GPU не используется**
```bash
# Проверьте OpenCL платформы
!clinfo | grep "Platform Name"
```

## 🎉 **Успех!**
Если все работает, вы увидите:
- **GPU ускорение** вместо CPU fallback
- **Векторизованные операции** с float4
- **Значительное ускорение** вычислений интерференции
- **Использование всех 2560 ядер** NVIDIA T4

## 📱 **Мониторинг GPU:**
```python
# В отдельной ячейке для мониторинга
!watch -n 1 nvidia-smi
```
