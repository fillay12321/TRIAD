use std::time::SystemTime;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

/// Статус транзакции
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Critical,
    High,
    Medium,
    Low,
}

/// Базовая структура транзакции
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub sender: String,
    pub receiver: String,
    pub amount: f64,
    pub timestamp: SystemTime,
    pub status: TransactionStatus,
    pub data: Vec<u8>,
}

impl Transaction {
    /// Создает новую транзакцию
    pub fn new(sender: String, receiver: String, amount: f64, data: Vec<u8>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: SystemTime::now(),
            sender,
            receiver,
            amount,
            status: TransactionStatus::Pending,
            data,
        }
    }

    /// Обновляет статус транзакции
    pub fn update_status(&mut self, status: TransactionStatus) {
        self.status = status;
    }

    /// Проверяет валидность транзакции
    pub fn is_valid(&self) -> bool {
        self.amount > 0.0 && !self.sender.is_empty() && !self.receiver.is_empty()
    }
}

/// Трейт для обработки транзакций
pub trait TransactionProcessor {
    fn process(&self, transaction: &mut Transaction) -> Result<(), String>;
    fn process_batch(&self, transactions: &mut [Transaction]) -> Result<(), String>;
    fn verify(&self, transaction: &Transaction) -> bool;
    fn create_wave_from_transaction(&self, transaction: &Transaction) -> crate::quantum::field::QuantumWave;
    fn create_probabilistic_decision(&self, analysis: &crate::quantum::interference::InterferenceAnalysis, transactions: &[Transaction]) -> crate::quantum::prob_ops::ProbabilisticOperation;
    fn calculate_transaction_quality(&self, transactions: &[Transaction]) -> f64;
    fn calculate_ttl_factor(&self) -> f64;
    fn calculate_sender_reputation(&self, transactions: &[Transaction]) -> f64;
    fn create_verification_decision(&self, analysis: &crate::quantum::interference::InterferenceAnalysis, transaction: &Transaction) -> crate::quantum::prob_ops::ProbabilisticOperation;
}

/// Трейт для хранения транзакций
pub trait TransactionStorage {
    fn save(&self, transaction: &Transaction) -> Result<(), String>;
    fn get(&self, id: &Uuid) -> Option<Transaction>;
    fn update(&self, transaction: &Transaction) -> Result<(), String>;
    fn delete(&self, id: &Uuid) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartContract {
    pub id: Uuid,
    pub creator: String,
    pub code: Vec<u8>,
    pub state: Vec<u8>,
    pub status: ContractStatus,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContractStatus {
    Pending,
    Deployed,
    Failed,
    Executed,
    Aborted,
}

pub trait SmartContractProcessor {
    fn deploy(&self, contract: &mut SmartContract) -> Result<(), String>;
    fn execute(&self, contract: &mut SmartContract, input: Vec<u8>) -> Result<Vec<u8>, String>;
    fn verify(&self, contract: &SmartContract) -> bool;
}

pub mod processor; 