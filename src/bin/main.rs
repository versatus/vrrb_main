use vrrb_lib::network::node::Node;
use vrrb_lib::account::{WalletAccount, AccountState};
use vrrb_lib::reward::RewardState;
use vrrb_lib::state::NetworkState;

// use vrrb_lib::network::protocol::{VrrbNetworkBehavior, build_transport};
use async_std::{io, task};
// use env_logger::{Builder, Env};
// use futures::prelude::*;
// use libp2p::kad::{Kademlia, record::store::MemoryStore};
// use libp2p::gossipsub::MessageId;
// use libp2p::gossipsub::{
//     GossipsubConfigBuilder,
//     GossipsubMessage, 
//     IdentTopic as Topic, 
//     MessageAuthenticity, 
//     ValidationMode,
//     Gossipsub,
// };
// use libp2p::identify::{IdentifyConfig, Identify};
// use libp2p::ping::{Ping, PingConfig};
// use libp2p::swarm::{Swarm};
// use libp2p::multiaddr::multiaddr;
// use libp2p::{identity, PeerId, Multiaddr};
// use std::collections::hash_map::DefaultHasher;
// use std::hash::{Hash, Hasher};
// use std::time::Duration;
use std::{
    error::Error,
    task::{Context, Poll},
    sync::{Arc, Mutex},
    thread,
};
// use std::collections::VecDeque;
// use rand::{Rng};
// use hex;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut account_state = Arc::new(Mutex::new(AccountState::start()));
    let mut network_state = Arc::new(Mutex::new(NetworkState::restore("test.db")));
    let mut reward_state = Arc::new(Mutex::new(RewardState::start(Arc::clone(&network_state))));
    let mut wallet = WalletAccount::new(Arc::clone(&account_state), Arc::clone(&network_state));
    // let node = Node::start(&mut wallet, &mut account_state, &mut network_state, &mut reward_state);

    Ok(())
}
//     Builder::from_env(Env::default().default_filter_or("info")).init();

//     let local_key = identity::Keypair::generate_ed25519();
//     let local_peer_id = PeerId::from(local_key.public());

//     let testnet_topic = Topic::new("test-net");
//     let txn_topic = Topic::new("txn");
//     let claim_topic = Topic::new("claim");
//     let block_topic = Topic::new("block");
//     let validator_topic = Topic::new("validator");
    

//     let mut swarm = {
//         let message_id_fn = |message: &GossipsubMessage| {
//             let mut s = DefaultHasher::new();
//             message.data.hash(&mut s);
//             MessageId::from(s.finish().to_string())
//         };

//         let gossipsub_config = GossipsubConfigBuilder::default()
//             .heartbeat_interval(Duration::from_secs(10))
//             .validation_mode(ValidationMode::Strict)
//             .message_id_fn(message_id_fn)
//             .build()
//             .expect("Valid config");
        
//         let mut gossipsub: Gossipsub = Gossipsub::new(
//             MessageAuthenticity::Signed(local_key.clone()), 
//             gossipsub_config).expect("Correct configuration");
        
//         gossipsub.subscribe(&testnet_topic).unwrap();
//         gossipsub.subscribe(&txn_topic).unwrap();
//         gossipsub.subscribe(&claim_topic).unwrap();
//         gossipsub.subscribe(&block_topic).unwrap();
//         gossipsub.subscribe(&validator_topic).unwrap();

//         let store = MemoryStore::new(local_peer_id);
//         let kademlia = Kademlia::new(local_peer_id, store);

//         let identify_config = IdentifyConfig::new(
//             "vrrb/test-net/1.0.0".to_string(),
//             local_key.public(),
//         );
//         let identify = Identify::new(identify_config);

//         let ping = Ping::new(PingConfig::new());
//         let queue: Arc<Mutex<VecDeque<GossipsubMessage>>> = Arc::new(Mutex::new(VecDeque::new()));

//         let behaviour = VrrbNetworkBehavior {
//             gossipsub,
//             identify,
//             kademlia,
//             ping,
//             queue,
//         };

//         let transport = build_transport(local_key).await.unwrap();

//         Swarm::new(transport, behaviour, local_peer_id)
//     };

//     let port = rand::thread_rng().gen_range(9292, 19292);
//     // Listen on all interfaces and whatever port the OS assigns
//     // TODO: Get the public IP of the node so external nodes can connect
//     // and only listen on this address.
//     let addr: Multiaddr = multiaddr!(Ip4([0,0,0,0]), Tcp(port as u16));
    
//     println!("{:?}", &addr);

//     swarm.listen_on(addr.clone()).unwrap();

//     swarm.behaviour_mut().kademlia.add_address(&local_peer_id, addr.clone());

//     if let Some(to_dial) = std::env::args().nth(1) {
//         let dialing = to_dial.clone();
//         match to_dial.parse() {
//             Ok(to_dial) => match swarm.dial_addr(to_dial) {
//                 Ok(_) => {
//                     println!("Dialed {:?}", dialing);
//                     },
//                 Err(e) => println!("Dial {:?} failed: {:?}", dialing, e)
//             },
//             Err(err) => println!("Failed to parse address to dial {:?}", err),
//         }
//     }

//     let mut stdin = io::BufReader::new(io::stdin()).lines();

//     task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {  
//         let atomic_queue = Arc::clone(&swarm.behaviour().queue);

//         thread::spawn(move || 
//             loop {
//                 match atomic_queue.lock().unwrap().pop_front() {
//                     Some(message) => println!("{:?}", message),
//                     None => {},
//                 }
//             });

//         loop {
//             match stdin.try_poll_next_unpin(cx)? {
//                 Poll::Ready(Some(line)) => {
//                     handle_input_line(&mut swarm.behaviour_mut(), line)
//                 },
//                 Poll::Ready(None) => panic!("Stdin closed"),
//                 Poll::Pending => break,
//             }        
//         }

//         loop {
//             match swarm.poll_next_unpin(cx) {
//                 Poll::Ready(Some(event)) => {
//                     match event {
//                         _ => println!("Event --> {:?}", event)
//                     }
//                 }
//                 Poll::Ready(None) | Poll::Pending => break
//             }
//         }
//         Poll::Pending
//     }))
// }


// fn handle_input_line(behaviour: &mut VrrbNetworkBehavior, line: String) {
//     // Message matching
//     //
//     // Insure topic is correct if so, then publish to topic, 
//     // if not then return a message to the local peer indicating 
//     // the message topic is incorrect.
//     // 
    
//     let message = hex::encode(line.as_bytes());
//     let mut commands = line.split(' ');

//     match commands.next() {
//         Some("NEW_TXN") | Some("UPD_TXN") => {
//             behaviour.gossipsub.publish(Topic::new("txn"), message).unwrap();
//         }
//         Some("CLM_HOM") | Some("CLM_SAL") | Some("CLM_ACQ") | Some("CLM_STK") => {
//             behaviour.gossipsub.publish(Topic::new("claim"), message).unwrap();
//         },
//         Some("NEW_BLK") | Some("GET_BLK") | Some("LST_BLK") => {
//             behaviour.gossipsub.publish(Topic::new("block"), message).unwrap();
//         },

//         Some("TXN_VAL") | Some("CLM_VAL") | Some("BLK_VAL") => {
//             behaviour.gossipsub.publish(Topic::new("validator"), message).unwrap();
//         },
//         Some("VRRB_IP") | Some("UPDST_P") => {
//             behaviour.gossipsub.publish(Topic::new("test-net"), message).unwrap();
//         },
//         Some(_) => {},
//         None => {},

//     }