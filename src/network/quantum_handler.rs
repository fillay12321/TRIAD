use async_trait::async_trait;
use uuid::Uuid;
use log::{info, debug, error, warn};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::network::types::{Message, PeerInfo};
use crate::network::error::NetworkError;
use crate::network::handler::MessageHandler;
use crate::quantum::consensus::{ConsensusNode, ConsensusMessage};
use crate::quantum::field::{QuantumWave, QuantumState};
use crate::transaction::{Transaction, SmartContract};
use crate::sharding::ShardEvent;
use crate::error_analysis::ErrorContext;
use crate::semantic::SemanticAction;

/// Квантовый обработчик сетевых сообщений
/// Интегрирует сетевой слой с квантовым консенсусом
#[derive(Clone)]
pub struct QuantumNetworkHandler {
    node_id: Uuid,
    consensus_node: Arc<Mutex<ConsensusNode>>,
    quantum_waves: Arc<Mutex<Vec<QuantumWave>>>,
    message_cache: Arc<Mutex<std::collections::HashMap<Uuid, Message>>>,
    interference_events: Arc<Mutex<Vec<NetworkInterferenceEvent>>>,
}

#[derive(Debug, Clone)]
pub struct NetworkInterferenceEvent {
    pub timestamp: std::time::Instant,
    pub wave_id: String,
    pub interference_strength: f64,
    pub affected_peers: Vec<Uuid>,
}

impl QuantumNetworkHandler {
    pub fn new(
        node_id: Uuid,
        consensus_node: Arc<Mutex<ConsensusNode>>,
    ) -> Self {
        Self {
            node_id,
            consensus_node,
            quantum_waves: Arc::new(Mutex::new(Vec::new())),
            message_cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
            interference_events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Создает ПОДПИСАННУЮ квантовую волну из сетевого сообщения (использует ключ узла консенсуса)
    async fn create_quantum_wave_from_message(&self, message: &Message) -> QuantumWave {
        let wave_id = format!("network_wave_{}_{}", self.node_id, message.id);
        
        // Создаем суперпозицию на основе типа сообщения
        let superposition = match &message.message_type {
            crate::network::MessageType::Transaction(_) => {
                vec![crate::quantum::field::StateVector {
                    value: num_complex::Complex::new(1.0, 0.0),
                    probability: 0.8,
                }]
            },
            crate::network::MessageType::Consensus(_) => {
                vec![crate::quantum::field::StateVector {
                    value: num_complex::Complex::new(0.0, 1.0),
                    probability: 0.9,
                }]
            },
            _ => {
                vec![crate::quantum::field::StateVector {
                    value: num_complex::Complex::new(0.5, 0.5),
                    probability: 0.5,
                }]
            }
        };

        // Подписываем волну ключом консенсус-узла
        let mut consensus = self.consensus_node.lock().await;
        QuantumWave::new_signed(
            wave_id,
            1.0, // амплитуда
            0.0, // фаза
            "network".to_string(),
            superposition,
            &consensus.secret_key,
        )
    }

    /// Обрабатывает квантовую интерференцию в сети
    async fn process_network_interference(&self, wave: &QuantumWave, peers: &[PeerInfo]) {
        let interference_event = NetworkInterferenceEvent {
            timestamp: std::time::Instant::now(),
            wave_id: wave.id.clone(),
            interference_strength: wave.amplitude,
            affected_peers: peers.iter().map(|p| p.id).collect(),
        };

        {
            let mut events = self.interference_events.lock().await;
            events.push(interference_event.clone());
            
            // Ограничиваем количество событий
            if events.len() > 100 {
                events.remove(0);
            }
        }

        debug!("Квантовая интерференция в сети: волна={}, сила={}, затронуто узлов={}", 
            wave.id, interference_event.interference_strength, peers.len());
    }

    /// Очищает устаревшие квантовые волны
    async fn cleanup_expired_waves(&self) {
        let mut waves = self.quantum_waves.lock().await;
        let now = std::time::Instant::now();
        
        waves.retain(|wave| {
            now.duration_since(wave.created_at) < wave.lifetime
        });
    }

    /// Пакетная обработка сообщений из кэша с батч-верификацией подписей (zebra_batch)
    async fn process_cached_messages_batch(&self) {
        // Снимаем снимок кэша, чтобы минимизировать время блокировки
        let messages: Vec<Message> = {
            let cache = self.message_cache.lock().await;
            cache.values().cloned().collect()
        };

        if messages.is_empty() { return; }

        // Собираем подписанные волны и верифицируем батчом
        let mut items: Vec<(QuantumWave, ed25519_dalek::VerifyingKey)> = Vec::with_capacity(messages.len());

        // Берем публик ключ локального узла (демо)
        let vk = {
            let consensus = self.consensus_node.lock().await;
            consensus.public_key.clone()
        };

        for msg in &messages {
            let wave = self.create_quantum_wave_from_message(msg).await;
            items.push((wave, vk.clone()));
        }

        // Подготовим ссылки для батч-верификации
        let wave_refs: Vec<(&QuantumWave, ed25519_dalek::VerifyingKey)> = items.iter().map(|(w, k)| (w, k.clone())).collect();

        let mut consensus = self.consensus_node.lock().await;
        if consensus.field.verify_waves_batch(&wave_refs) {
            // Добавляем все волны в поле
            for (wave, _k) in items.into_iter() {
                let _ = consensus.field.add_signed_wave(wave.id.clone(), wave, &vk);
            }
        } else {
            warn!("Batch verification failed for {} cached messages", messages.len());
        }
    }

    /// Получает статистику квантовой сети
    pub async fn get_quantum_network_stats(&self) -> QuantumNetworkStats {
        let waves = self.quantum_waves.lock().await;
        let events = self.interference_events.lock().await;
        let cache = self.message_cache.lock().await;

        QuantumNetworkStats {
            active_waves: waves.len(),
            interference_events: events.len(),
            cached_messages: cache.len(),
            total_wave_amplitude: waves.iter().map(|w| w.amplitude).sum(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuantumNetworkStats {
    pub active_waves: usize,
    pub interference_events: usize,
    pub cached_messages: usize,
    pub total_wave_amplitude: f64,
}

#[async_trait]
impl MessageHandler for QuantumNetworkHandler {
    async fn handle_message(&mut self, from: Uuid, message: Message) -> Result<(), NetworkError> {
        info!("[QuantumNetworkHandler] Получено сообщение от {}: тип={:?}, id={}", from, message.message_type, message.id);

        // Создаем квантовую волну из сообщения
        let wave = self.create_quantum_wave_from_message(&message).await;
        
        // Добавляем волну в квантовое поле ТОЛЬКО после проверки подписи
        {
            let mut consensus = self.consensus_node.lock().await;
            let vk = consensus.public_key.clone();
            if let Err(e) = consensus.field.add_signed_wave(wave.id.clone(), wave.clone(), &vk) {
                error!("Отклонена волна из сообщения {}: {}", message.id, e);
                return Err(NetworkError::Internal(e));
            }
        }

        // Кэшируем сообщение
        {
            let mut cache = self.message_cache.lock().await;
            cache.insert(message.id, message.clone());
            
            // Ограничиваем размер кэша
            if cache.len() > 1000 {
                cache.clear();
            }
        }

        // Триггерим пакетную обработку, когда в кэше наберется минимум 16 сообщений
        if { self.message_cache.lock().await.len() } >= 16 {
            self.process_cached_messages_batch().await;
        }

        // Обрабатываем сообщение в зависимости от типа
        match message.message_type {
            crate::network::MessageType::Transaction(transaction) => {
                self.handle_transaction_message(from, transaction).await?;
            },
            crate::network::MessageType::Consensus(consensus_msg) => {
                self.handle_consensus_message(from, consensus_msg).await?;
            },
            crate::network::MessageType::SmartContract(contract) => {
                self.handle_smart_contract_message(from, contract).await?;
            },
            crate::network::MessageType::ShardEvent(event) => {
                self.handle_shard_event_message(from, event).await?;
            },
            crate::network::MessageType::ErrorEvent(error_ctx) => {
                self.handle_error_event_message(from, error_ctx).await?;
            },
            crate::network::MessageType::SemanticAction(action) => {
                self.handle_semantic_action_message(from, action).await?;
            },
            crate::network::MessageType::Error(error_str) => {
                warn!("Получена ошибка от узла {}: {}", from, error_str);
            },
        }

        // Очищаем устаревшие волны
        self.cleanup_expired_waves().await;

        Ok(())
    }

    async fn handle_peer_connected(&mut self, peer_info: PeerInfo) -> Result<(), NetworkError> {
        info!("[QuantumNetworkHandler] Новый peer подключен: {} ({})", peer_info.name, peer_info.id);

        // Создаем ПОДПИСАННУЮ квантовую волну для нового узла
        let wave = QuantumWave::new_signed(
            format!("peer_connected_{}", peer_info.id),
            1.0,
            0.0,
            "peer_connection".to_string(),
            vec![crate::quantum::field::StateVector {
                value: num_complex::Complex::new(1.0, 0.0),
                probability: 1.0,
            }],
            &self.consensus_node.lock().await.secret_key,
        );

        // Добавляем в квантовое поле после проверки подписи
        {
            let mut consensus = self.consensus_node.lock().await;
            let vk = consensus.public_key.clone();
            if let Err(e) = consensus.field.add_signed_wave(wave.id.clone(), wave.clone(), &vk) {
                error!("Отклонена волна peer_connected: {}", e);
                return Err(NetworkError::Internal(e));
            }
        }

        // Обрабатываем интерференцию с другими узлами
        let peers = vec![peer_info.clone()];
        self.process_network_interference(&wave, &peers).await;

        Ok(())
    }

    async fn handle_peer_disconnected(&mut self, peer_id: Uuid) -> Result<(), NetworkError> {
        info!("[QuantumNetworkHandler] Peer отключился: {}", peer_id);

        // Создаем ПОДПИСАННУЮ волну отключения
        let wave = QuantumWave::new_signed(
            format!("peer_disconnected_{}", peer_id),
            0.5, // Уменьшенная амплитуда для отключения
            0.0,
            "peer_disconnection".to_string(),
            vec![crate::quantum::field::StateVector {
                value: num_complex::Complex::new(0.5, 0.0),
                probability: 0.5,
            }],
            &self.consensus_node.lock().await.secret_key,
        );

        // Добавляем в квантовое поле после проверки подписи
        {
            let mut consensus = self.consensus_node.lock().await;
            let vk = consensus.public_key.clone();
            if let Err(e) = consensus.field.add_signed_wave(wave.id.clone(), wave, &vk) {
                error!("Отклонена волна peer_disconnected: {}", e);
            }
        }

        Ok(())
    }
}

impl QuantumNetworkHandler {
    async fn handle_transaction_message(&self, from: Uuid, transaction: Transaction) -> Result<(), NetworkError> {
        debug!("Обработка транзакции от узла {}: {}", from, transaction.id);
        
        // Создаем квантовое состояние из транзакции
        let quantum_state = QuantumState::new(
            transaction.id.to_string(),
            transaction.amount as f64,
            0.0,
            "transaction".to_string(),
            vec![crate::quantum::field::StateVector {
                value: num_complex::Complex::new(transaction.amount as f64, 0.0),
                probability: 1.0,
            }],
        );

        // Обрабатываем через консенсус: подписываем полезную нагрузку
        let raw = bincode::serialize(&transaction).unwrap_or_default();
        let signature = {
            let consensus = self.consensus_node.lock().await;
            let sig = consensus.sign_message(&raw);
            sig.to_bytes().to_vec()
        };
        let consensus_message = ConsensusMessage {
            sender_id: from.to_string(),
            state_id: transaction.id.to_string(),
            raw_data: raw,
            signature,
        };

        {
            let mut consensus = self.consensus_node.lock().await;
            // Используем публичный ключ локального узла (в демо мы отправитель)
            let vk = consensus.public_key.clone();
            if let Err(e) = consensus.process_message(consensus_message, &vk) {
                error!("Ошибка обработки транзакции в консенсусе: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_consensus_message(&self, from: Uuid, consensus_msg: ConsensusMessage) -> Result<(), NetworkError> {
        debug!("Обработка консенсусного сообщения от узла {}: {}", from, consensus_msg.state_id);
        
        {
            let mut consensus = self.consensus_node.lock().await;
            // Создаем фиктивный публичный ключ для демо
            let dummy_public_key = consensus.public_key.clone();
            if let Err(e) = consensus.process_message(consensus_msg, &dummy_public_key) {
                error!("Ошибка обработки консенсусного сообщения: {}", e);
            }
        }

        Ok(())
    }

    async fn handle_smart_contract_message(&self, from: Uuid, contract: SmartContract) -> Result<(), NetworkError> {
        debug!("Обработка смарт-контракта от узла {}: {}", from, contract.id);
        
        // Создаем квантовое состояние из контракта
        let quantum_state = QuantumState::new(
            contract.id.to_string(),
            1.0,
            0.0,
            "smart_contract".to_string(),
            vec![crate::quantum::field::StateVector {
                value: num_complex::Complex::new(1.0, 0.0),
                probability: 1.0,
            }],
        );

        Ok(())
    }

    async fn handle_shard_event_message(&self, from: Uuid, event: ShardEvent) -> Result<(), NetworkError> {
        debug!("Обработка события шарда от узла {}: {:?}", from, event);
        Ok(())
    }

    async fn handle_error_event_message(&self, from: Uuid, error_ctx: ErrorContext) -> Result<(), NetworkError> {
        debug!("Обработка ошибки от узла {}: {}", from, error_ctx.description);
        Ok(())
    }

    async fn handle_semantic_action_message(&self, from: Uuid, action: SemanticAction) -> Result<(), NetworkError> {
        debug!("Обработка семантического действия от узла {}: {:?}", from, action);
        Ok(())
    }
} 