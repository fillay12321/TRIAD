use triad::{
    network::{NetworkNode, NetworkMessage, NodeConfig, MessageType},
    transaction::{Transaction, SmartContract, TransactionStatus, ContractStatus},
    sharding::ShardEvent,
    semantic::SemanticAction,
};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use std::time::SystemTime;

const NUM_NODES: usize = 3;
const TX_PER_NODE: usize = 2;
const CONTRACTS_PER_NODE: usize = 1;
const SHARD_EVENTS: usize = 2;

fn main() {
    // 1. Network initialization from multiple nodes
    let mut nodes: Vec<NetworkNode> = (0..NUM_NODES)
        .map(|i| NetworkNode::new(NodeConfig {
            id: format!("node_{}", i),
            address: format!("127.0.0.1:90{}", i),
            initial_peers: vec![],
        }))
        .collect();

    println!("=== Step 1: Initializing {} nodes ===", NUM_NODES);
    for node in &nodes {
        println!("  - {} @ {}", node.id, node.address);
    }

    // 2. Transaction generation and broadcasting
    println!("\n=== Step 2: Generating and broadcasting transactions ===");
    for i in 0..NUM_NODES {
        for j in 0..TX_PER_NODE {
            let tx = Transaction {
                id: Uuid::new_v4(),
                sender: format!("user_{}", i),
                receiver: format!("user_{}", (i + 1) % NUM_NODES),
                amount: 10.0 * (j as f64 + 1.0),
                timestamp: SystemTime::now(),
                status: TransactionStatus::Pending,
                data: vec![],
            };
            println!("  - Node {} sends transaction {} -> {} amount {}", i, tx.sender, tx.receiver, tx.amount);
            for node in &mut nodes {
                node.process_message(NetworkMessage { message_type: MessageType::Transaction(tx.clone()) });
            }
        }
    }

    // 3. Smart contract generation and broadcasting
    println!("\n=== Step 3: Generating and broadcasting smart contracts ===");
    for i in 0..NUM_NODES {
        for j in 0..CONTRACTS_PER_NODE {
            let contract = SmartContract {
                id: Uuid::new_v4(),
                creator: format!("dev_{}", i),
                code: vec![1,2,3,4, (i+j) as u8],
                state: vec![],
                status: ContractStatus::Pending,
                timestamp: SystemTime::now(),
            };
            println!("  - Node {} deploys contract from {}", i, contract.creator);
            for node in &mut nodes {
                node.process_message(NetworkMessage { message_type: MessageType::SmartContract(contract.clone()) });
            }
        }
    }

    // 4. Sharding event generation and broadcasting
    println!("\n=== Step 4: Generating and broadcasting sharding events ===");
    for i in 0..SHARD_EVENTS {
        let event = ShardEvent {
            shard_id: format!("shard_{}", i),
            event_type: if i % 2 == 0 { "split".to_string() } else { "merge".to_string() },
            node_id: format!("node_{}", i % NUM_NODES),
            timestamp: SystemTime::now(),
            details: Some(format!("{} event for demo", if i % 2 == 0 { "Split" } else { "Merge" })),
        };
        println!("  - Event: {} for {} initiated by {}", event.event_type, event.shard_id, event.node_id);
        for node in &mut nodes {
            node.process_message(NetworkMessage { message_type: MessageType::ShardEvent(event.clone()) });
        }
    }

    // 5. Artificial errors and semantic actions
    println!("\n=== Step 5: Artificial errors and semantic actions ===");
    for i in 0..NUM_NODES {
        let error_msg = format!("Test error from node {}", i);
        nodes[i].process_message(NetworkMessage { message_type: MessageType::Error(error_msg) });
        let semantic_action = SemanticAction {
            entity_id: format!("node_{}", i),
            action_type: "manual_semantic_action".to_string(),
            details: Some("Manual semantic action for demonstration".to_string()),
        };
        nodes[i].process_message(NetworkMessage { message_type: MessageType::SemanticAction(semantic_action) });
    }

    // 6. Detailed output of each node's state
    println!("\n=== Step 6: Node summary ===");
    for (i, node) in nodes.iter().enumerate() {
        println!("\n--- Node {} ({}) ---", i, node.id);
        println!("Transactions: {}", node.transactions.len());
        for t in &node.transactions {
            println!("  - {:?}", t);
        }
        println!("Smart contracts: {}", node.contracts.len());
        for c in &node.contracts {
            println!("  - {:?}", c);
        }
        println!("Shard events: {}", node.shard_events.len());
        for s in &node.shard_events {
            println!("  - {:?}", s);
        }
        println!("Errors: {}", node.errors.len());
        for e in &node.errors {
            println!("  - {:?}", e);
        }
        println!("Semantic actions: {}", node.semantic_actions.len());
        for a in &node.semantic_actions {
            println!("  - {:?}", a);
        }
    }
    println!("\n=== End of comprehensive TRIAD demo ===");
} 