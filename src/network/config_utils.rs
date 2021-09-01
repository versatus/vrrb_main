#[allow(unused_imports)]
use crate::account::AccountState;
use crate::network::command_utils::Command;
use crate::network::protocol::{build_transport, VrrbNetworkBehavior};
use libp2p::gossipsub::MessageId;
use libp2p::gossipsub::{
    Gossipsub, GossipsubConfigBuilder, GossipsubMessage, IdentTopic as Topic, MessageAuthenticity,
    ValidationMode,
};
use libp2p::identify::{Identify, IdentifyConfig};
use libp2p::kad::{record::store::MemoryStore, Kademlia};
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::Swarm;
use libp2p::{identity, PeerId};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::Sender;
use std::time::Duration;

pub const MAX_TRANSMIT_SIZE: usize = 2000000;

pub async fn configure_swarm(
    message_sender: Sender<GossipsubMessage>,
    command_sender: Sender<Command>,
) -> Swarm<VrrbNetworkBehavior> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    let message_id_fn = |message: &GossipsubMessage| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        MessageId::from(s.finish().to_string())
    };

    let gossipsub_config = GossipsubConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(2))
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
    let txn_topic = Topic::new("txn");
    let claim_topic = Topic::new("claim");
    let block_topic = Topic::new("block");
    let validator_topic = Topic::new("validator");

    gossipsub.subscribe(&testnet_topic).unwrap();
    gossipsub.subscribe(&txn_topic).unwrap();
    gossipsub.subscribe(&claim_topic).unwrap();
    gossipsub.subscribe(&block_topic).unwrap();
    gossipsub.subscribe(&validator_topic).unwrap();
    let store = MemoryStore::new(local_peer_id);
    let kademlia = Kademlia::new(local_peer_id, store);
    let identify_config =
        IdentifyConfig::new("vrrb/test-net/1.0.0".to_string(), local_key.public());

    let identify = Identify::new(identify_config);
    let ping = Ping::new(PingConfig::new());

    let behaviour = VrrbNetworkBehavior {
        gossipsub,
        identify,
        kademlia,
        ping,
        command_sender: command_sender.clone(),
        message_sender: message_sender.clone(),
    };

    let transport = build_transport(local_key).await.unwrap();

    Swarm::new(transport, behaviour, local_peer_id)
}
