/// Модуль для работы с квантовыми вычислениями
pub mod quantum;

/// Модуль для шардинга данных
pub mod sharding;

/// Сетевой модуль для обмена данными между узлами
pub mod network;

/// Экспортируем основные структуры и функции из модулей
pub use quantum::field::QuantumField;
pub use quantum::consensus::QuantumConsensus; 