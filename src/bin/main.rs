use vrrb_lib::network::protocol::{VrrbNetworkBehavior, build_transport};
use async_std::{io, task};
use env_logger::{Builder, Env};
use futures::prelude::*;
use libp2p::kad::{Kademlia, record::store::MemoryStore};
use libp2p::gossipsub::MessageId;
use libp2p::gossipsub::{
    GossipsubConfigBuilder,
    GossipsubMessage, 
    IdentTopic as Topic, 
    MessageAuthenticity, 
    ValidationMode,
    Gossipsub,
    GossipsubEvent
};
use libp2p::identify::{IdentifyConfig, Identify};
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::{Swarm};
use libp2p::multiaddr::multiaddr;
use libp2p::{identity, PeerId, Multiaddr};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use std::{
    error::Error,
    task::{Context, Poll},
};
use rand::{Rng};

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {

    Builder::from_env(Env::default().default_filter_or("info")).init();

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    let topic = Topic::new("test-net");

    let mut swarm = {
        let message_id_fn = |message: &GossipsubMessage| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            MessageId::from(s.finish().to_string())
        };

        let gossipsub_config = GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10))
            .validation_mode(ValidationMode::Strict)
            .message_id_fn(message_id_fn)
            .build()
            .expect("Valid config");
        
        let mut gossipsub: Gossipsub = Gossipsub::new(
            MessageAuthenticity::Signed(local_key.clone()), 
            gossipsub_config).expect("Correct configuration");
        
        gossipsub.subscribe(&topic).unwrap();

        let store = MemoryStore::new(local_peer_id);
        let mut kademlia = Kademlia::new(local_peer_id, store);

        let identify_config = IdentifyConfig::new(
            "vrrb/test-net/1.0.0".to_string(),
            local_key.public(),
        );
        let identify = Identify::new(identify_config);

        let ping = Ping::new(PingConfig::new());

        let behaviour = VrrbNetworkBehavior {
            gossipsub,
            identify,
            kademlia,
            ping
        };

        let transport = build_transport(local_key).await.unwrap();

        Swarm::new(transport, behaviour, local_peer_id)
    };

    let port = rand::thread_rng().gen_range(9292, 19292);
    // Listen on all interfaces and whatever port the OS assigns
    let addr: Multiaddr = multiaddr!(Ip4([0,0,0,0]), Tcp(port as u16));
    
    println!("{:?}", &addr);

    swarm.listen_on(addr.clone()).unwrap();

    if let Some(to_dial) = std::env::args().nth(1) {
        let dialing = to_dial.clone();
        match to_dial.parse() {
            Ok(to_dial) => match swarm.dial_addr(to_dial) {
                Ok(_) => {
                    println!("Dialed {:?}", dialing);
                    },
                Err(e) => println!("Dial {:?} failed: {:?}", dialing, e)
            },
            Err(err) => println!("Failed to parse address to dial {:?}", err),
        }
    }

    let mut stdin = io::BufReader::new(io::stdin()).lines();

    task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
        loop {
            match stdin.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(line)) => {
                    handle_input_line(&mut swarm.behaviour_mut(), line, &topic)
                },
                Poll::Ready(None) => panic!("Stdin closed"),
                Poll::Pending => break,
            }        
        }

        loop {
            match swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => {
                    match event {
                        _ => println!("Event --> {:?}", event)
                    }
                }
                Poll::Ready(None) | Poll::Pending => break
            }
        }
        Poll::Pending
    }))
}


fn handle_input_line(behaviour: &mut VrrbNetworkBehavior, line: String, topic: &Topic) {
    let commands = line.split(' ');

    for command in commands.into_iter() {
        println!("{}", command);
    }
}