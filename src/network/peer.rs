use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use uuid::Uuid;
use crate::network::types::PeerInfo;
use crate::network::error::NetworkError;

/// Время, после которого узел считается неактивным
pub const PEER_TIMEOUT: Duration = Duration::from_secs(60);

/// Менеджер узлов для отслеживания подключенных пиров
#[derive(Debug)]
pub struct PeerManager {
    /// Мапа известных узлов: ключ - ID узла, значение - информация об узле
    peers: HashMap<Uuid, PeerInfo>,
}

impl PeerManager {
    /// Создаёт новый менеджер узлов
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }
    
    /// Добавляет или обновляет информацию об узле
    pub fn add_peer(&mut self, peer_info: PeerInfo) -> bool {
        let is_new = !self.peers.contains_key(&peer_info.id);
        self.peers.insert(peer_info.id, peer_info);
        is_new
    }
    
    /// Возвращает информацию об узле по ID
    pub fn get_peer(&self, peer_id: &Uuid) -> Option<&PeerInfo> {
        self.peers.get(peer_id)
    }
    
    /// Возвращает изменяемую ссылку на информацию об узле по ID
    pub fn get_peer_mut(&mut self, peer_id: &Uuid) -> Option<&mut PeerInfo> {
        self.peers.get_mut(peer_id)
    }
    
    /// Удаляет узел из списка известных
    pub fn remove_peer(&mut self, peer_id: &Uuid) -> Option<PeerInfo> {
        self.peers.remove(peer_id)
    }
    
    /// Возвращает список всех известных узлов
    pub fn get_all_peers(&self) -> Vec<PeerInfo> {
        self.peers.values().cloned().collect()
    }
    
    /// Возвращает список только активных узлов
    pub fn get_active_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .values()
            .filter(|peer| peer.is_active(PEER_TIMEOUT))
            .cloned()
            .collect()
    }
    
    /// Возвращает количество известных узлов
    pub fn count(&self) -> usize {
        self.peers.len()
    }
    
    /// Обновляет время последнего взаимодействия с узлом
    pub fn update_last_seen(&mut self, peer_id: &Uuid) -> Result<(), NetworkError> {
        if let Some(peer) = self.peers.get_mut(peer_id) {
            peer.update_last_seen();
            Ok(())
        } else {
            Err(NetworkError::PeerNotFound(*peer_id))
        }
    }
    
    /// Получает адрес узла по его ID
    pub fn get_peer_address(&self, peer_id: &Uuid) -> Result<SocketAddr, NetworkError> {
        if let Some(peer) = self.peers.get(peer_id) {
            Ok(peer.address)
        } else {
            Err(NetworkError::PeerNotFound(*peer_id))
        }
    }
    
    /// Очищает неактивные узлы
    pub fn cleanup_inactive_peers(&mut self) -> Vec<Uuid> {
        let inactive: Vec<Uuid> = self.peers
            .iter()
            .filter(|(_, peer)| !peer.is_active(PEER_TIMEOUT))
            .map(|(id, _)| *id)
            .collect();
        
        for id in &inactive {
            self.peers.remove(id);
        }
        
        inactive
    }
    
    /// Ищет узел по адресу
    pub fn find_peer_by_address(&self, address: &SocketAddr) -> Option<&PeerInfo> {
        self.peers
            .values()
            .find(|peer| peer.address == *address)
    }
} 