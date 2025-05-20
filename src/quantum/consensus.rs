use crate::quantum::field::{QuantumField, QuantumState, StateVector};
use crate::quantum::interference::InterferenceEngine;
use crate::quantum::prob_ops::{ProbabilisticOperation, OperationOutcome};
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::sync::RwLock;
use rayon::prelude::*;
use std::collections::HashMap;
use serde::ser::SerializeStruct;
use serde::de::{Deserializer, Visitor};
use std::fmt;

#[derive(Debug, Clone)]
pub struct ConsensusNode {
    pub id: String,
    pub shard_id: usize,
    pub field: QuantumField,
    pub engine: InterferenceEngine,
    cache: Arc<RwLock<HashMap<String, bool>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusMessage {
    pub sender_id: String,
    pub state_id: String,
    pub raw_data: Vec<u8>,
}

impl ConsensusNode {
    pub fn new(id: String, shard_id: usize) -> Self {
        Self {
            id,
            shard_id,
            field: QuantumField::new(),
            engine: InterferenceEngine::new(100, 0.95),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn process_message(&mut self, message: ConsensusMessage) -> Result<QuantumState, String> {
        // Проверяем кэш
        if let Ok(cache) = self.cache.read() {
            if cache.contains_key(&message.state_id) {
                return self.field.get_state(&message.state_id);
            }
        }

        // Создаем вероятностную операцию для обработки данных
        let op = ProbabilisticOperation {
            description: "Data processing".to_string(),
            outcomes: vec![
                OperationOutcome {
                    label: "Processed".to_string(),
                    probability: 0.8,
                    value: Some(serde_json::json!({
                        "processed": true,
                        "data": message.raw_data
                    })),
                },
                OperationOutcome {
                    label: "Failed".to_string(),
                    probability: 0.2,
                    value: Some(serde_json::json!({
                        "processed": false,
                        "error": "Processing failed"
                    })),
                },
            ],
        };

        // Выполняем вероятностную обработку
        let outcome = op.execute();
        
        // Создаем квантовое состояние на основе результата обработки
        let state = QuantumState {
            id: message.state_id.clone(),
            shard_id: self.shard_id,
            amplitude: 1.0,
            phase: 0.0,
            superposition: vec![
                StateVector {
                    value: message.raw_data,
                    probability: if outcome.label == "Processed" { 0.8 } else { 0.2 },
                },
            ],
        };

        // Добавляем состояние в поле
        self.field.add_state(message.state_id.clone(), state.clone());
        
        // Сохраняем в кэш
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(message.state_id, true);
        }
        
        Ok(state)
    }

    pub fn check_interference(&self, state_id: &str) -> Result<bool, String> {
        // Проверяем кэш
        if let Ok(cache) = self.cache.read() {
            if let Some(&result) = cache.get(state_id) {
                return Ok(result);
            }
        }

        let state = self.field.get_state(state_id)?;
        
        // Создаем второе состояние для интерференции
        let interference_state = QuantumState {
            id: state.id.clone(),
            shard_id: state.shard_id,
            amplitude: 0.8,
            phase: 0.0,
            superposition: state.superposition.clone(),
        };

        // Рассчитываем паттерн интерференции
        let pattern = self.engine.calculate_interference_pattern(&vec![
            state.clone(),
            interference_state,
        ]);

        // Анализируем интерференцию
        let analysis = self.engine.analyze_interference(&pattern);

        // Проверяем наличие конструктивной интерференции
        let result = !analysis.constructive_points.is_empty();

        // Сохраняем в кэш
        if let Ok(mut cache) = self.cache.write() {
            cache.insert(state_id.to_string(), result);
        }

        Ok(result)
    }
}

// Функция для параллельной обработки сообщений
pub fn process_messages_parallel(nodes: &mut [ConsensusNode], message: ConsensusMessage) -> Result<(), String> {
    nodes.par_iter_mut().for_each(|node| {
        node.process_message(message.clone()).unwrap();
    });
    Ok(())
}

// Функция для параллельной проверки интерференции
pub fn check_interference_parallel(nodes: &[ConsensusNode], state_id: &str) -> Result<bool, String> {
    let results: Vec<bool> = nodes.par_iter()
        .map(|node| node.check_interference(state_id).unwrap())
        .collect();
    
    Ok(results.iter().all(|&x| x))
} 