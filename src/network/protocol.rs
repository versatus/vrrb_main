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
    ping::{self, Ping, PingEvent},
    swarm::NetworkBehaviourEventProcess,
    tcp::TcpConfig,
    websocket::WsConfig,
    yamux::YamuxConfig,
    NetworkBehaviour, PeerId, Transport,
};
use log::info;
use std::io::Error;
use std::time::Duration;
use tokio::sync::broadcast;

#[derive(NetworkBehaviour)]
pub struct VrrbNetworkBehavior {
    pub gossipsub: Gossipsub,
    pub identify: Identify,
    pub kademlia: Kademlia<MemoryStore>,
    pub ping: Ping,
    #[behaviour(ignore)]
    pub command_sender: broadcast::Sender<Command>,
    #[behaviour(ignore)]
    pub message_sender: broadcast::Sender<GossipsubMessage>,
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for VrrbNetworkBehavior {
    // called when 'identify'
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Received { peer_id, info } => {

                for addr in info.listen_addrs {
                    self.kademlia.add_address(&peer_id, addr);
                }
                self.kademlia.bootstrap().unwrap();
            }
            IdentifyEvent::Error { peer_id, error } => {
                info!(target: "protocol_error", "Encountered an error: {:?} -> {:?}", error, peer_id);
            }
            _ => {}
        }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, event: GossipsubEvent) {
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
        use ping::handler::PingFailure;
        match event {
            PingEvent { result, peer } => {
                match result {
                    Ok(_) => {}
                    Err(failure) => {
                        match failure {
                            PingFailure::Timeout => {
                                info!(target: "failed ping", "pinged {} and was a failure", &peer.to_string());
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
                                info!(target: "failed ping", "pinged {} and was a failure", &peer.to_string());
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
    fn inject_event(&mut self, message: KademliaEvent) {
        match message {
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
