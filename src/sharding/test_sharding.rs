use crate::quantum::consensus::{ConsensusNode, ConsensusMessage};
use super::{Shard, assign_shard};
use rayon::prelude::*;

#[test]
fn test_sharding_parallel_processing() {
    let num_shards = 4;
    let nodes_per_shard = 5;
    let messages_per_node = 10;

    // Create shards
    let mut shards: Vec<Shard> = (0..num_shards)
        .map(|shard_id| {
            let nodes = (0..nodes_per_shard)
                .map(|i| {
                    let node_id = format!("shard{}_node{}", shard_id, i);
                    let shard_id = assign_shard(&node_id, num_shards);
                    ConsensusNode::new(node_id, shard_id)
                })
                .collect();
            Shard::new(shard_id, nodes)
        })
        .collect();

    // For each shard — its own list of messages for each node
    let mut all_shard_messages: Vec<Vec<Vec<ConsensusMessage>>> = vec![
        vec![vec![]; nodes_per_shard]; num_shards
    ];
    for shard_id in 0..num_shards {
        for node_idx in 0..nodes_per_shard {
            let messages = (0..messages_per_node)
                .map(|op| ConsensusMessage {
                    sender_id: format!("shard{}_node{}", shard_id, node_idx),
                    state_id: format!("state{}_{}_{}", shard_id, node_idx, op),
                    raw_data: vec![op as u8, (op + 1) as u8, (op + 2) as u8],
                })
                .collect();
            all_shard_messages[shard_id][node_idx] = messages;
        }
    }

    // Parallel processing of shards
    shards.par_iter_mut().enumerate().for_each(|(shard_id, shard)| {
        shard.process_messages(&all_shard_messages[shard_id]);
    });
} 