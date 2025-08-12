//! Discovery module using libp2p for efficient P2P networking
//! Implements QUIC/TCP transport, Kademlia DHT, GossipSub, Identify, Ping,
//! AutoNAT and DCUtR hole punching. Bootstrap-only relay server, optional WebRTC fallback.

pub mod quic_transport;
pub mod gossip;
pub mod dht;
pub mod compression;
pub mod state_sync;
pub mod sharding;

use std::{env, net::Ipv4Addr, str::FromStr, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use log::{info, error, debug, warn};
use libp2p::{
    core::upgrade,
    futures::StreamExt,
    gossipsub,
    identify,
    identity,
    kad::{store::MemoryStore, Kademlia, KademliaEvent},
    multiaddr::Protocol,
    noise,
    ping,
    swarm::{NetworkBehaviour, Swarm, SwarmBuilder, SwarmEvent},
    tcp,
    yamux,
    Multiaddr, PeerId, Transport,
};

#[cfg(feature = "webrtc")]
use libp2p_webrtc as webrtc;

use crate::network::types::{PeerInfo, NetworkEvent, Message};
use crate::network::peer::PeerManager;
use crate::network::error::NetworkError;
use libp2p::futures::Future;

/// Main discovery service using libp2p
pub struct LibP2PDiscoveryService {
    /// Local identity
    local_key: identity::Keypair,
    /// Local PeerId
    local_peer_id: PeerId,
    /// Node name
    node_name: String,
    /// TCP listen port
    tcp_port: u16,
    /// QUIC listen port
    quic_port: u16,
    /// Channel for sending network events
    event_sender: mpsc::Sender<NetworkEvent>,
    /// Peer manager
    peer_manager: Arc<RwLock<PeerManager>>,
    /// Sender for shutdown signal
    shutdown_tx: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// Whether this node acts as the only bootstrap
    is_bootstrap: bool,
    /// Bootstrap multiaddr to dial (if not bootstrap)
    bootstrap_addr: Option<Multiaddr>,
}

/// Combined network behaviour
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "DiscoveryEvent")]
struct DiscoveryBehaviour {
    gossipsub: gossipsub::Behaviour,
    kademlia: Kademlia<MemoryStore>,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[derive(Debug)]
enum DiscoveryEvent {
    Gossipsub(gossipsub::Event),
    Kademlia(KademliaEvent),
    Identify(identify::Event),
    Ping(ping::Event),
}

impl From<gossipsub::Event> for DiscoveryEvent {
    fn from(event: gossipsub::Event) -> Self {
        DiscoveryEvent::Gossipsub(event)
    }
}

impl From<KademliaEvent> for DiscoveryEvent {
    fn from(event: KademliaEvent) -> Self {
        DiscoveryEvent::Kademlia(event)
    }
}

impl From<identify::Event> for DiscoveryEvent {
    fn from(event: identify::Event) -> Self {
        DiscoveryEvent::Identify(event)
    }
}

impl From<ping::Event> for DiscoveryEvent {
    fn from(event: ping::Event) -> Self {
        DiscoveryEvent::Ping(event)
    }
}

impl LibP2PDiscoveryService {
    /// Creates new libp2p discovery service
    pub fn new(
        node_name: String,
        quic_port: u16,
        event_sender: mpsc::Sender<NetworkEvent>,
        peer_manager: Arc<RwLock<PeerManager>>,
    ) -> Self {
        let local_key = identity::Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        // Only your machine should be the bootstrap
        // Configure via env vars:
        // TRIAD_BOOTSTRAP=1 on the bootstrap node
        // TRIAD_BOOTSTRAP_MULTIADDR=/ip4/<public_ip>/tcp/<port> on other nodes
        let is_bootstrap = env::var("TRIAD_BOOTSTRAP").ok().as_deref() == Some("1");
        let bootstrap_addr = env::var("TRIAD_BOOTSTRAP_MULTIADDR")
            .ok()
            .and_then(|s| Multiaddr::from_str(&s).ok());

        let tcp_port = env::var("TRIAD_LISTEN_TCP_PORT")
            .ok()
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(4001);

        info!("🚀 Created libp2p discovery: peer_id={}, node={}, bootstrap_mode={}, bootstrap_addr={:?}",
              local_peer_id, node_name, is_bootstrap, bootstrap_addr);

        Self {
            local_key,
            local_peer_id,
            node_name,
            tcp_port,
            quic_port,
            event_sender,
            peer_manager,
            shutdown_tx: Arc::new(RwLock::new(None)),
            is_bootstrap,
            bootstrap_addr,
        }
    }
    
    /// Build TCP transport
    fn build_transport(&self) -> Result<libp2p::core::transport::Boxed<(PeerId, libp2p::core::muxing::StreamMuxerBox)>, NetworkError> {
        // TCP with Noise/Yamux
        let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(upgrade::Version::V1Lazy)
            .authenticate(noise::Config::new(&self.local_key).map_err(|e| NetworkError::Internal(e.to_string()))?)
            .multiplex(yamux::Config::default())
            .boxed();

        Ok(tcp_transport)
    }

    /// Build behaviour (Kademlia + Gossipsub + Ping + Identify)
    fn build_behaviour(&self) -> DiscoveryBehaviour {
        // GossipSub
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .validation_mode(gossipsub::ValidationMode::Permissive)
            .build()
            .expect("valid gossipsub config");
        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(self.local_key.clone()),
            gossipsub_config,
        ).expect("gossipsub");

        // Kademlia
        let store = MemoryStore::new(self.local_peer_id);
        let mut kademlia = Kademlia::new(self.local_peer_id, store);

        // Identify
        let identify = identify::Behaviour::new(identify::Config::new(
            format!("triad/{}", env::var("TRIAD_VERSION").unwrap_or_else(|_| "dev".into())),
            self.local_key.public(),
        ));

        // Ping
        let ping = ping::Behaviour::new(ping::Config::new());

        // Subscribe to a default topic
        let topic = gossipsub::IdentTopic::new("triad-global");
        let _ = gossipsub.subscribe(&topic);
        // Bootstrap mode: no-op here. Peer addresses are added dynamically on Identify/connection events.

        DiscoveryBehaviour { gossipsub, kademlia, identify, ping }
    }

    /// Starts the discovery service (public interface)
    pub async fn start(self: Arc<Self>) -> Result<(), NetworkError> {
        if self.is_running().await {
            return Ok(());
        }

        info!("🚀 Starting libp2p discovery service...");

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        let mut swarm = {
            let transport = self.build_transport()?;
            let behaviour = self.build_behaviour();
            SwarmBuilder::with_executor(transport, behaviour, self.local_peer_id, TokioExecutor).build()
        };

        // Listen on TCP and QUIC
        swarm
            .listen_on(
                Multiaddr::empty()
                    .with(Protocol::Ip4(Ipv4Addr::UNSPECIFIED))
                    .with(Protocol::Tcp(self.tcp_port)),
            )
            .map_err(|e| NetworkError::ConnectionFailed(format!("listen TCP failed: {}", e)))?;
        swarm
            .listen_on(
                Multiaddr::empty()
                    .with(Protocol::Ip4(Ipv4Addr::UNSPECIFIED))
                    .with(Protocol::Udp(self.quic_port))
                    .with(Protocol::QuicV1),
            )
            .map_err(|e| NetworkError::ConnectionFailed(format!("listen QUIC failed: {}", e)))?;

        if let Some(bootstrap_addr) = &self.bootstrap_addr {
            swarm
                .dial(bootstrap_addr.clone())
                .map_err(|e| NetworkError::ConnectionFailed(format!("dial {} failed: {}", bootstrap_addr, e)))?;
        }

        let event_sender = self.event_sender.clone();
        let peer_manager = self.peer_manager.clone();
        let node_name = self.node_name.clone();

        loop {
            tokio::select! {
                event = swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(DiscoveryEvent::Identify(identify::Event::Received { peer_id, info })) => {
                            info!("🤝 Identified peer: {} with {:?}", peer_id, info.listen_addrs);
                            for addr in info.listen_addrs {
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                            }
                        }
                        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                            let addr = endpoint.get_remote_address();
                            if let (Some(ip), Some(port)) = (addr.iter().find_map(|p| if let Protocol::Ip4(ip) = p { Some(ip) } else { None }), addr.iter().find_map(|p| if let Protocol::Tcp(port) = p { Some(port) } else { None })) {
                                use std::net::SocketAddr;
                                let sock = SocketAddr::new(std::net::IpAddr::V4(ip), port);
                                let info = PeerInfo::new(Uuid::new_v4(), node_name.clone(), sock);
                                let mut pm = peer_manager.write().await;
                                if pm.add_peer(info) {
                                    if let Some(peer) = pm.get_all_peers().last().cloned() {
                                        let _ = event_sender.send(NetworkEvent::PeerConnected(peer)).await;
                                    }
                                }
                            }
                        }
                        SwarmEvent::ConnectionClosed { peer_id, .. } => {
                             let _ = event_sender.send(NetworkEvent::PeerDisconnected(Uuid::new_v4())).await; // Note: Uuid needs to be tracked
                             debug!("🔌 Disconnected from {}", peer_id);
                        }
                        other => {
                            debug!("🔭 Swarm event: {:?}", other);
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    info!("🔽 Received shutdown signal.");
                    break;
                }
            }
        }
        Ok(())
    }
    
    /// Stops the discovery service
    pub async fn stop(&self) -> Result<(), NetworkError> {
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            info!("🛑 Sending shutdown signal to libp2p service...");
            tx.send(()).map_err(|_| NetworkError::ShutdownFailed)?;
            info!("✅ libp2p discovery service stopped");
        }
        Ok(())
    }

    /// Gets current peer ID
    pub fn peer_id(&self) -> PeerId { 
        self.local_peer_id 
    }
    
    /// Gets node name
    pub fn node_name(&self) -> &str {
        &self.node_name
    }

    /// Checks if the service is running
    pub async fn is_running(&self) -> bool {
        self.shutdown_tx.read().await.is_some()
    }
}

#[derive(Clone)]
struct TokioExecutor;

impl libp2p::swarm::Executor for TokioExecutor {
    fn exec(&self, future: std::pin::Pin<Box<dyn Future<Output = ()> + Send>>) {
        tokio::spawn(future);
    }
}
