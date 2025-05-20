pub mod field;
pub mod interference;
pub mod prob_ops;
pub mod consensus;

pub use field::{QuantumField, QuantumState, StateVector, InterferencePoint};
pub use interference::{InterferenceEngine, InterferenceAnalysis};
pub use prob_ops::{ProbabilisticOperation, OperationOutcome};

use std::sync::Arc;
use tokio::sync::RwLock;

pub struct QuantumSystem {
    field: Arc<RwLock<QuantumField>>,
    interference_engine: InterferenceEngine,
}

impl QuantumSystem {
    pub fn new() -> Self {
        Self {
            field: Arc::new(RwLock::new(QuantumField::new())),
            interference_engine: InterferenceEngine::new(1000, 0.95),
        }
    }

    pub async fn add_state(&self, key: String, state: QuantumState) {
        let mut field = self.field.write().await;
        field.add_state(key, state);
    }

    pub async fn measure(&self, key: &str) -> Option<StateVector> {
        let field = self.field.read().await;
        field.measure(key)
    }

    pub async fn get_interference_pattern(&self) -> Vec<InterferencePoint> {
        let field = self.field.read().await;
        let states: Vec<QuantumState> = field.states.values().cloned().collect();
        self.interference_engine.calculate_interference_pattern(&states)
    }

    pub async fn analyze_interference(&self) -> InterferenceAnalysis {
        let pattern = self.get_interference_pattern().await;
        self.interference_engine.analyze_interference(&pattern)
    }
} 