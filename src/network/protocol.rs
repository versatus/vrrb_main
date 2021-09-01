use libp2p::{
    core::{
        muxing::StreamMuxerBox,
        transport::upgrade::Version,
        transport::Boxed,
        upgrade::SelectUpgrade,
    },
    kad::record::store::MemoryStore,
    kad::{
        Kademlia,
        KademliaEvent,
        QueryResult,
    },
    swarm::{NetworkBehaviourEventProcess}, 
    gossipsub::{
        Gossipsub, 
        GossipsubEvent,
        GossipsubMessage,
    },
    identify::{
        Identify,  
        IdentifyEvent
    },
    websocket::WsConfig,
    dns::DnsConfig,
    identity,
    noise,
    ping::{
        self, 
        Ping, 
        PingEvent,
    },
    NetworkBehaviour,
    tcp::TcpConfig,
    yamux::YamuxConfig,
    mplex::MplexConfig,
    PeerId, 
    Transport,
};
use crate::network::command_utils::Command;
use std::io::Error;
use std::time::Duration;
use std::sync::mpsc::Sender;
use log::{info};

#[derive(NetworkBehaviour)]
pub struct VrrbNetworkBehavior {
    pub gossipsub: Gossipsub,
    pub identify: Identify,
    pub kademlia: Kademlia<MemoryStore>,
    pub ping: Ping,
    #[behaviour(ignore)]
    pub command_sender: Sender<Command>,
    #[behaviour(ignore)]
    pub message_sender: Sender<GossipsubMessage>,
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for VrrbNetworkBehavior {
    // called when 'identify'
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Received {
                peer_id,
                info,
            } => {
                // If a new peer is received add them to the DHT and ??send Identity back??
                // Bootstrap the new node.
                // Add listening addresses to the routing table -> Local Addresses will be
                // excluded in the future, as the listening address must be external so that
                // peers will only have a single address.
                for addr in info.listen_addrs {
                    self.kademlia.add_address(&peer_id, addr);
                }
                self.kademlia.bootstrap().unwrap();
                // After a new peer has been bootstrapped, have a `trusted` node send them a
                // message with the most recent state.
                // This command will trigger the structuring and sending of a state update message.

                // if let Some(command) = Command::from_str(&format!("SENDSTE {}", peer_id.to_string())) {
                //     if let Err(e) = self.command_sender.send(command) {
                //         println!("Error sending message to command thread: {:?}", e);
                //     };
                // }
            },
            IdentifyEvent::Error {
                peer_id,
                error,
            } => {
                info!(target: "protocol_error", "Encountered an error: {:?} -> {:?}", error, peer_id);
            },
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
                message
            } => {
                if let Err(_) = self.message_sender.send(message) {
                    println!("Error sending message to message handling thread");
                };
            },
            _ => {}
        }
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, event: PingEvent) {
        use ping::handler::{PingFailure};
        match event {
            PingEvent {
                result,
                peer
            } => {
                match result {
                    Ok(_success) => {},
                    Err(failure) => {
                        match failure {
                            PingFailure::Timeout => {
                                //TODO: Dial again and try again, keep track of failures and
                                // if it fails three times then drop peer.
                                self.kademlia.remove_peer(&peer);
                            },
                            PingFailure::Other { .. } => {
                                self.kademlia.remove_peer(&peer);
                            }
                        }
                    }
                }
            },
        }
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, message: KademliaEvent) {
        match message {
            KademliaEvent::QueryResult { result, .. } => {
                match result {
                    QueryResult::Bootstrap(Ok(ok)) => {
                        self.kademlia.get_closest_peers(ok.peer);

                    },
                    QueryResult::Bootstrap(Err(err)) => {
                        println!("Encountered an error while trying to bootstrap peer: {:?}", err);
                    },
                    QueryResult::GetClosestPeers(Ok(_)) => {

                    },
                    QueryResult::GetClosestPeers(Err(err)) => {
                        println!("Encountered an error while trying to get closest peers: {:?}", err);
                    },
                    QueryResult::GetProviders(Ok(ok)) => {
                        for peer in ok.providers {
                            println!("Provider: {:?}", peer)
                        }
                    },
                    QueryResult::GetProviders(Err(err)) => {
                        println!("Encountered an error while trying to get providers: {:?}", err);
                    },
                    QueryResult::GetRecord(Ok(ok)) => {
                        for record in ok.records {
                            println!("Got record: {:?}", record);
                        }
                    },
                    QueryResult::GetRecord(Err(err)) => {
                        println!("Encountered error while trying to get record: {:?}", err);
                    },
                    QueryResult::PutRecord(Ok(ok)) => {
                        println!("Put record: {:?}", ok.key);
                    },
                    QueryResult::PutRecord(Err(err)) => {
                        println!("Encountered errorw while trying to put record: {:?}", err);
                    },
                    QueryResult::StartProviding(Ok(ok)) => {
                        println!("Started Providing: {:?}", ok.key);
                    },
                    QueryResult::StartProviding(Err(err)) => {
                        println!("Encountered an error while trying to start providing: {:?}", err);
                    },
                    QueryResult::RepublishProvider(Ok(ok)) => {
                        println!("Republishing provider: {:?}", ok.key);
                    },
                    QueryResult::RepublishProvider(Err(err)) => {
                        println!("Encountered an error while trying to repbulish a provider: {:?}", err);
                    },
                    QueryResult::RepublishRecord(Ok(ok)) => {
                        println!("Republishing record: {:?}", ok.key);
                    },
                    QueryResult::RepublishRecord(Err(err)) => {
                        println!("Encountered an error while attempting to republish record: {:?}", err);
                    }
                }
            }, 
            _ => {}
        }
    }
}

pub async fn build_transport(
    key_pair: identity::Keypair
) -> Result<Boxed<(PeerId, StreamMuxerBox)>, Error> 
{
    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&key_pair)
        .unwrap();
    
    let noise_config = noise::NoiseConfig::xx(noise_keys).into_authenticated();
    let yamux_config = YamuxConfig::default();
    let mplex_config = MplexConfig::default();
    
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
        .timeout(Duration::from_secs(20))
        .boxed()
    )
}