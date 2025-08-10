use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, error, debug};

use super::http_server::{HttpMessage, ConsensusRequest, ConsensusResponse, HealthResponse};

#[derive(Debug, Clone)]
pub struct HttpClient {
    pub node_id: String,
    pub base_url: String,
    pub client: Client,
}

impl HttpClient {
    pub fn new(node_id: String, base_url: String) -> Self {
        Self {
            node_id,
            base_url,
            client: Client::new(),
        }
    }

    pub async fn health_check(&self, peer_url: &str) -> Result<HealthResponse, String> {
        let url = format!("http://{}/health", peer_url);
        debug!("🔍 Health check: {}", url);
        
        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<HealthResponse>().await {
                        Ok(health) => {
                            info!("✅ Peer {} is healthy: {}:{}", peer_url, health.node_id, health.port);
                            Ok(health)
                        },
                        Err(e) => Err(format!("Failed to parse health response: {}", e)),
                    }
                } else {
                    Err(format!("Health check failed with status: {}", response.status()))
                }
            },
            Err(e) => Err(format!("Health check request failed: {}", e)),
        }
    }

    pub async fn send_consensus_request(
        &self,
        peer_url: &str,
        round: usize,
        transactions: Vec<Vec<u8>>,
        shard_id: usize,
    ) -> Result<ConsensusResponse, String> {
        let url = format!("http://{}/consensus", peer_url);
        let request = ConsensusRequest {
            round,
            transactions,
            shard_id,
        };
        
        debug!("📤 Sending consensus request to {}: round {}, shard {}", peer_url, round, shard_id);
        
        match self.client.post(&url).json(&request).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<ConsensusResponse>().await {
                        Ok(consensus) => {
                            info!("✅ Consensus response from {}: success={}, processed={}", 
                                  peer_url, consensus.success, consensus.processed_count);
                            Ok(consensus)
                        },
                        Err(e) => Err(format!("Failed to parse consensus response: {}", e)),
                    }
                } else {
                    Err(format!("Consensus request failed with status: {}", response.status()))
                }
            },
            Err(e) => Err(format!("Consensus request failed: {}", e)),
        }
    }

    pub async fn send_message(
        &self,
        peer_url: &str,
        message_type: String,
        payload: Vec<u8>,
        target_shard: Option<usize>,
    ) -> Result<(), String> {
        let url = format!("http://{}/message", peer_url);
        let message = HttpMessage {
            sender_id: self.node_id.clone(),
            message_type,
            payload,
            target_shard,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        debug!("📨 Sending message to {}: type={}", peer_url, message.message_type);
        
        match self.client.post(&url).json(&message).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("✅ Message sent successfully to {}", peer_url);
                    Ok(())
                } else {
                    Err(format!("Message send failed with status: {}", response.status()))
                }
            },
            Err(e) => Err(format!("Message send failed: {}", e)),
        }
    }

    pub async fn discover_peers(&self, peer_url: &str) -> Result<Vec<String>, String> {
        let url = format!("http://{}/peers", peer_url);
        debug!("🔍 Discovering peers from: {}", url);
        
        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<Vec<String>>().await {
                        Ok(peers) => {
                            info!("✅ Discovered {} peers from {}", peers.len(), peer_url);
                            Ok(peers)
                        },
                        Err(e) => Err(format!("Failed to parse peers response: {}", e)),
                    }
                } else {
                    Err(format!("Peer discovery failed with status: {}", response.status()))
                }
            },
            Err(e) => Err(format!("Peer discovery failed: {}", e)),
        }
    }

    pub async fn broadcast_to_peers(
        &self,
        peer_urls: &[String],
        message_type: String,
        payload: Vec<u8>,
        target_shard: Option<usize>,
    ) -> Vec<Result<(), String>> {
        let mut results = Vec::new();
        
        for peer_url in peer_urls {
            let result = self.send_message(peer_url, message_type.clone(), payload.clone(), target_shard).await;
            results.push(result);
        }
        
        results
    }
}
