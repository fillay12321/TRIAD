use rand::Rng;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilisticOperation {
    pub description: String,
    pub outcomes: Vec<OperationOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationOutcome {
    pub label: String,
    pub probability: f64, // Должна быть сумма 1.0 по всем исходам
    pub value: Option<serde_json::Value>,
}

impl ProbabilisticOperation {
    pub fn execute(&self) -> &OperationOutcome {
        let mut rng = rand::thread_rng();
        let random = rng.gen_range(0.0..1.0);
        let mut cumulative = 0.0;
        for outcome in &self.outcomes {
            cumulative += outcome.probability;
            if random <= cumulative {
                return outcome;
            }
        }
        // Если не сработало (например, из-за неточности суммы), возвращаем последний исход
        self.outcomes.last().expect("Нет исходов для операции")
    }
} 