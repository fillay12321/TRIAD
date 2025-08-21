use std::collections::{HashMap, HashSet};
use rayon::prelude::*;
use uuid::Uuid;

pub struct Dag {
    nodes: HashMap<Uuid, Vec<Uuid>>, // tx_id -> parents
}

impl Dag {
    pub fn new() -> Self { Self { nodes: HashMap::new() } }

    pub fn add_transaction(&mut self, tx_id: Uuid, parents: Vec<Uuid>) {
        self.nodes.insert(tx_id, parents);
    }

    pub fn batch_add_transactions(&mut self, txs: &[(Uuid, Vec<Uuid>)]) {
        for (id, parents) in txs {
            self.add_transaction(*id, parents.clone());
        }
    }

    pub fn add_weighted_transaction(&mut self, tx_id: Uuid, parents: Vec<(Uuid, f64)>) {
        let parent_ids: Vec<Uuid> = parents.iter().map(|(id, _)| *id).collect();
        self.nodes.insert(tx_id, parent_ids);
        // Store weights separately if needed
    }

    pub fn validate_parallel(&self, tx_ids: &[Uuid]) -> Vec<bool> {
        tx_ids.par_iter().map(|id| self.validate(id)).collect()
    }

    fn validate(&self, _tx_id: &Uuid) -> bool {
        // Validation logic with interference check
        true
    }

    // --- Новые методы ---
    /// Получить все id транзакций DAG
    pub fn all_tx_ids(&self) -> Vec<Uuid> {
        self.nodes.keys().cloned().collect()
    }

    /// Получить родителей транзакции
    pub fn parents(&self, tx_id: &Uuid) -> Option<&Vec<Uuid>> {
        self.nodes.get(tx_id)
    }

    /// Получить детей транзакции
    pub fn children(&self, tx_id: &Uuid) -> Vec<Uuid> {
        self.nodes.iter()
            .filter_map(|(child, parents)| if parents.contains(tx_id) { Some(*child) } else { None })
            .collect()
    }

    /// Глубина DAG (максимальная длина пути от корня до листа)
    pub fn depth(&self) -> usize {
        let mut max_depth = 0;
        for tx in self.nodes.keys() {
            let d = self.depth_from(tx, &mut HashSet::new());
            if d > max_depth { max_depth = d; }
        }
        max_depth
    }
    fn depth_from(&self, tx_id: &Uuid, visited: &mut HashSet<Uuid>) -> usize {
        if !visited.insert(*tx_id) { return 0; }
        match self.parents(tx_id) {
            Some(parents) if !parents.is_empty() => {
                1 + parents.iter().map(|p| self.depth_from(p, visited)).max().unwrap_or(0)
            },
            _ => 1,
        }
    }

    /// Проверка наличия цикла (DAG должен быть ацикличен)
    pub fn has_cycle(&self) -> bool {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        for tx in self.nodes.keys() {
            if self.cycle_dfs(tx, &mut visited, &mut stack) {
                return true;
            }
        }
        false
    }
    fn cycle_dfs(&self, tx_id: &Uuid, visited: &mut HashSet<Uuid>, stack: &mut HashSet<Uuid>) -> bool {
        if !visited.insert(*tx_id) {
            return false;
        }
        if !stack.insert(*tx_id) {
            return true;
        }
        if let Some(parents) = self.parents(tx_id) {
            for p in parents {
                if self.cycle_dfs(p, visited, stack) {
                    return true;
                }
            }
        }
        stack.remove(tx_id);
        false
    }
} 