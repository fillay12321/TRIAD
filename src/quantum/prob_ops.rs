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
    pub probability: f64, // The sum of all outcomes must be 1.0
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
        // If it didn't work (for example, due to sum inaccuracy), return the last outcome
        self.outcomes.last().expect("No outcomes for the operation")
    }
} 