#[allow(unused_imports)]
use crate::account::AccountState;
use crate::network::command_utils::Command;
use crate::network::protocol::{build_transport, VrrbNetworkBehavior};
use core::num::NonZeroU32;
use libp2p::gossipsub::MessageId;
use libp2p::gossipsub::{
    Gossipsub, GossipsubConfigBuilder, GossipsubMessage, IdentTopic as Topic, MessageAuthenticity,
    ValidationMode,
};
use libp2p::identify::{Identify, IdentifyConfig};
use libp2p::kad::{record::store::MemoryStore, Kademlia};
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::Swarm;
use libp2p::{identity::Keypair, PeerId};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::sync::mpsc;

pub const MAX_TRANSMIT_SIZE: usize = 2000000;

pub async fn configure_swarm(
    message_sender: mpsc::UnboundedSender<GossipsubMessage>,
    command_sender: mpsc::UnboundedSender<Command>,
    local_peer_id: PeerId,
    local_key: Keypair,
    pubkey: String,
    address: String,
    event_path: String,
) -> Swarm<VrrbNetworkBehavior> {
    let message_id_fn = |message: &GossipsubMessage| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        MessageId::from(s.finish().to_string())
    };

    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(1))
        .history_length(5)
        .history_gossip(3)
        .mesh_n(6)
        .mesh_n_low(4)
        .mesh_n_high(12)
        .gossip_lazy(6)
        .gossip_factor(0.25)
        .fanout_ttl(Duration::from_secs(60))
        .check_explicit_peers_ticks(150)
        .do_px()
        .published_message_ids_cache_time(Duration::from_secs(5))
        .validation_mode(ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .flood_publish(true)
        .max_transmit_size(MAX_TRANSMIT_SIZE)
        .build()
        .expect("Valid config");

    let mut gossipsub: Gossipsub = Gossipsub::new(
        MessageAuthenticity::Signed(local_key.clone()),
        gossipsub_config,
    )
    .expect("Correct configuration");

    let testnet_topic = Topic::new("test-net");
    gossipsub.subscribe(&testnet_topic).unwrap();

    let store = MemoryStore::new(local_peer_id);
    let kademlia = Kademlia::new(local_peer_id, store);

    let identify_config =
        IdentifyConfig::new("vrrb/test-net/0.1.0".to_string(), local_key.public());
    let identify = Identify::new(identify_config);

    let ping_config = PingConfig::new();
    ping_config
        .with_interval(Duration::from_secs(20))
        .with_max_failures(NonZeroU32::new(3).unwrap())
        .with_timeout(Duration::from_secs(20));

    let ping = Ping::new(PingConfig::new());

    let behaviour = VrrbNetworkBehavior {
        gossipsub,
        identify,
        kademlia,
        ping,
        command_sender: command_sender.clone(),
        message_sender: message_sender.clone(),
        pubkey,
        address,
        path: event_path.clone(),
    };

    let transport = build_transport(local_key).await.unwrap();

    Swarm::new(transport, behaviour, local_peer_id)
}
