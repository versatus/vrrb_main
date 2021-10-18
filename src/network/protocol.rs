use crate::network::command_utils::Command;
use libp2p::{
    core::{
        muxing::StreamMuxerBox, transport::upgrade::Version, transport::Boxed,
        upgrade::SelectUpgrade,
    },
    dns::DnsConfig,
    gossipsub::{Gossipsub, GossipsubEvent, GossipsubMessage},
    identify::{Identify, IdentifyEvent},
    identity,
    kad::record::store::MemoryStore,
    kad::{Kademlia, KademliaEvent, QueryResult},
    mplex::MplexConfig,
    noise,
    ping::handler::PingFailure,
    ping::{Ping, PingEvent},
    swarm::NetworkBehaviourEventProcess,
    tcp::TcpConfig,
    websocket::WsConfig,
    yamux::YamuxConfig,
    NetworkBehaviour, PeerId, Transport,
};
use log::info;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fs;
use std::io::Error;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum VrrbNetworkEvent {
    VrrbStarted,
    VrrbProtocolEvent {
        event: String
    }
}

#[derive(NetworkBehaviour)]
pub struct VrrbNetworkBehavior {
    pub gossipsub: Gossipsub,
    pub identify: Identify,
    pub kademlia: Kademlia<MemoryStore>,
    pub ping: Ping,
    #[behaviour(ignore)]
    pub command_sender: mpsc::UnboundedSender<Command>,
    #[behaviour(ignore)]
    pub message_sender: mpsc::UnboundedSender<GossipsubMessage>,
    #[behaviour(ignore)]
    pub pubkey: String,
    #[behaviour(ignore)]
    pub address: String,
    #[behaviour(ignore)]
    pub path: String,
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for VrrbNetworkBehavior {
    // called when 'identify'
    fn inject_event(&mut self, event: IdentifyEvent) {
        if let Err(_) = write_to_json(self.path.clone(), &event) {
            info!("Error writing to json in identify event");
        };
        match event {
            IdentifyEvent::Received { peer_id, info } => {
                for addr in &info.listen_addrs {
                    self.kademlia.add_address(&peer_id, addr.clone());
                }
                self.kademlia.bootstrap().unwrap();
            }
            _ => {}
        }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, event: GossipsubEvent) {
        if let Err(_) = write_to_json(self.path.clone(), &event) {
            info!("Error writing to json in GossipsubEvent");
        };
        match event {
            GossipsubEvent::Message {
                propagation_source: _peer_id,
                message_id: _id,
                message,
            } => {
                if let Err(_) = self.message_sender.send(message) {
                    println!("Error sending message to message handling thread");
                };
            }
            _ => {}
        }
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, event: PingEvent) {
        if let Err(_) = write_to_json(self.path.clone(), &event) {
            info!("Error writing to json in PingEvent");
        }
        match event {
            PingEvent { result, peer } => {
                match result {
                    Ok(_) => {}
                    Err(failure) => {
                        match failure {
                            PingFailure::Timeout => {
                                //TODO: Dial again and try again, keep track of failures and
                                // if it fails three times then drop peer.
                                self.kademlia.remove_peer(&peer);
                                {
                                    println!(
                                        "Error sending remove peer command to command receiver"
                                    );
                                }
                            }
                            PingFailure::Other { .. } => {
                                self.kademlia.remove_peer(&peer);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, event: KademliaEvent) {
        if let Err(_) = write_to_json(self.path.clone(), &event) {
            info!("Error writing to json in Kademlia Event");
        }
        match event {
            KademliaEvent::QueryResult { result, .. } => match result {
                QueryResult::Bootstrap(Ok(ok)) => {
                    self.kademlia.get_closest_peers(ok.peer);
                }
                QueryResult::Bootstrap(Err(err)) => {
                    println!(
                        "Encountered an error while trying to bootstrap peer: {:?}",
                        err
                    );
                }
                QueryResult::GetClosestPeers(Ok(_)) => {}
                QueryResult::GetClosestPeers(Err(err)) => {
                    println!(
                        "Encountered an error while trying to get closest peers: {:?}",
                        err
                    );
                }
                QueryResult::GetProviders(Ok(ok)) => {
                    for peer in ok.providers {
                        println!("Provider: {:?}", peer)
                    }
                }
                QueryResult::GetProviders(Err(err)) => {
                    println!(
                        "Encountered an error while trying to get providers: {:?}",
                        err
                    );
                }
                QueryResult::GetRecord(Ok(ok)) => {
                    for record in ok.records {
                        println!("Got record: {:?}", record);
                    }
                }
                QueryResult::GetRecord(Err(err)) => {
                    println!("Encountered error while trying to get record: {:?}", err);
                }
                QueryResult::PutRecord(Ok(ok)) => {
                    println!("Put record: {:?}", ok.key);
                }
                QueryResult::PutRecord(Err(err)) => {
                    println!("Encountered errorw while trying to put record: {:?}", err);
                }
                QueryResult::StartProviding(Ok(ok)) => {
                    println!("Started Providing: {:?}", ok.key);
                }
                QueryResult::StartProviding(Err(err)) => {
                    println!(
                        "Encountered an error while trying to start providing: {:?}",
                        err
                    );
                }
                QueryResult::RepublishProvider(Ok(ok)) => {
                    println!("Republishing provider: {:?}", ok.key);
                }
                QueryResult::RepublishProvider(Err(err)) => {
                    println!(
                        "Encountered an error while trying to repbulish a provider: {:?}",
                        err
                    );
                }
                QueryResult::RepublishRecord(Ok(ok)) => {
                    println!("Republishing record: {:?}", ok.key);
                }
                QueryResult::RepublishRecord(Err(err)) => {
                    println!(
                        "Encountered an error while attempting to republish record: {:?}",
                        err
                    );
                }
            },
            _ => {}
        }
    }
}

pub async fn build_transport(
    key_pair: identity::Keypair,
) -> Result<Boxed<(PeerId, StreamMuxerBox)>, Error> {
    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&key_pair)
        .unwrap();
    let noise_config = noise::NoiseConfig::xx(noise_keys).into_authenticated();
    let yamux_config = YamuxConfig::default();
    let mut mplex_config = MplexConfig::new();
    mplex_config
        .set_max_num_streams(2000)
        .set_max_buffer_size(200000000)
        .set_split_send_size(1000000);

    let transport = {
        let tcp = TcpConfig::new().nodelay(true);
        let dns_tcp = DnsConfig::system(tcp).await.unwrap();
        let ws_dns_tcp = WsConfig::new(dns_tcp.clone());
        dns_tcp.or_transport(ws_dns_tcp)
    };

    Ok(transport
        .upgrade(Version::V1)
        .authenticate(noise_config)
        .multiplex(SelectUpgrade::new(yamux_config, mplex_config))
        .timeout(Duration::from_secs(30))
        .boxed())
}

pub fn write_to_json<T: Debug>(path: String, event: &T) -> Result<(), serde_json::Error> {
    let content = fs::read_to_string(path.clone());
    if let Ok(string) = content {
        let result: Result<Vec<VrrbNetworkEvent>, serde_json::Error> =
            serde_json::from_str(&string);
        if let Ok(mut events) = result {
            let new_event = get_event(event);
            events.push(new_event);
            if events.len() > 100 {
                events.remove(0);
            }
            let json_vec = serde_json::to_vec(&events);
            if let Ok(json) = json_vec {
                if let Err(e) = fs::write(path.clone(), json) {
                    info!("Error writing event to events.json: {:?}", e);
                }
            }
        } else {
            let new_event = get_event(event);
            let events = vec![new_event];
            let json_vec = serde_json::to_vec(&events);
            if let Ok(json) = json_vec {
                if let Err(e) = fs::write(path.clone(), json) {
                    info!("Error writing event to events.json: {:?}", e);
                }
            }
        }
    }
    Ok(())
}

pub fn get_event<T: Debug>(event: &T) -> VrrbNetworkEvent {
    let event_string = format!("{:?}", event);
    VrrbNetworkEvent::VrrbProtocolEvent { event: event_string }
}