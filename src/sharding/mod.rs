use crate::quantum::consensus::{ConsensusNode, ConsensusMessage};
use rayon::prelude::*;

pub struct Shard {
    pub id: usize,
    pub nodes: Vec<ConsensusNode>,
}

impl Shard {
    pub fn new(id: usize, nodes: Vec<ConsensusNode>) -> Self {
        Self { id, nodes }
    }

    /// Обработка сообщений внутри шарда (каждый узел получает свою очередь)
    pub fn process_messages(&mut self, all_messages: &[Vec<ConsensusMessage>]) {
        self.nodes.par_iter_mut().enumerate().for_each(|(node_idx, node)| {
            for message in &all_messages[node_idx] {
                let _ = node.process_message(message.clone());
            }
        });
    }
}

/// Распределяет сообщения по шардам по хешу state_id
pub fn assign_shard(state_id: &str, num_shards: usize) -> usize {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;
    let mut hasher = DefaultHasher::new();
    state_id.hash(&mut hasher);
    (hasher.finish() as usize) % num_shards
}

#[cfg(test)]
pub mod test_sharding; 