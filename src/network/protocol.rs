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
use std::io::Error;
use std::time::Duration;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(NetworkBehaviour)]
pub struct VrrbNetworkBehavior {
    pub gossipsub: Gossipsub,
    pub identify: Identify,
    pub kademlia: Kademlia<MemoryStore>,
    pub ping: Ping,
    #[behaviour(ignore)]
    pub queue: Arc<Mutex<VecDeque<GossipsubMessage>>>
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
                println!("Received Identity of Peer: {:?} -> Info: {:?}", &peer_id, &info);
                // Add listening addresses to the routing table -> Local Addresses will be
                // excluded in the future, as the listening address must be external so that
                // peers will only have a single address.
                for addr in info.listen_addrs {
                    self.kademlia.add_address(&peer_id, addr);
                }
                self.kademlia.bootstrap().unwrap();
            },
            IdentifyEvent::Sent {
                peer_id
            } => {
                println!("Sent Identify info: {:?}", peer_id);
                // Nothing more to do if identity info is sent.
            },
            IdentifyEvent::Pushed {
                peer_id
            } => {
                println!("Pushed Peer info: {:?}", peer_id);
                // Nothing more to do if peer info is pushed.
            },
            IdentifyEvent::Error {
                peer_id,
                error,
            } => {
                println!("Encountered an error: {:?} -> {:?}", error, peer_id);
            }
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

                let _header = &String::from_utf8_lossy(&message.data)[0..7];
                let _data = &String::from_utf8_lossy(&message.data)[9..];
                //TODO: Push all message information instead of just the message itself
                //TODO: Convert the message to a mutable byte array since byte array's implement copy
                // and can be dereferenced and changed.
                self.queue.lock().unwrap().push_back(message)
            },
            GossipsubEvent::Subscribed {
                peer_id, topic
            } => {
                println!("Peer subscribed: {:?} to topic: {:?}", peer_id, topic);
            
            },
            GossipsubEvent::Unsubscribed {
                peer_id, topic
            } => {
                println!("Peer unsubscribed: {:?} from topic: {:?}", peer_id, topic);
            },
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
            KademliaEvent::QueryResult { id, result, stats } => {
                println!("Received query result:\n id: {:?} \n result: {:?}, stats: {:?}", &id, &result, &stats);
                match result {
                    QueryResult::Bootstrap(Ok(ok)) => {
                        println!("Peer: {:?} bootstrapped. Num remaining: {:?}", ok.peer, ok.num_remaining);
                        self.kademlia.get_closest_peers(ok.peer);
                    },
                    QueryResult::Bootstrap(Err(err)) => {
                        println!("Encountered an error while trying to bootstrap peer: {:?}", err);
                    },
                    QueryResult::GetClosestPeers(Ok(ok)) => {
                        for (idx, peer) in ok.peers.iter().enumerate() {
                            println!("Next closest peer: {:?} -> {:?}", ok.key[idx], peer);
                        }
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

            KademliaEvent::RoutingUpdated { peer, addresses, old_peer } => {
                println!("Peer Routing has updated: {:?} now at -> Peer Id: {:?} -> Address: {:?}",
                    old_peer, peer, addresses);
                // Dial the listening address to connect.
            },
            KademliaEvent::UnroutablePeer { peer } => {
                println!("Peer {:?} is unroutable", peer);
                // If the peer is unroutable request the listening address and
                // add it to the routing table.
            },
            KademliaEvent::RoutablePeer { peer, address } => {
                println!("Peer ID {:?} -> Address: {:?}", peer, address);
                // If the peer is routable but not added to the routing table
                // because the table is full, it's insertion into the routing table
                // should be pending the least recently disconnected peer or should be
                // added to the table.
                // If the peer is to be unconditionally added to the routing table, then the
                // peer can be added using the add_address() method.
            },
            KademliaEvent::PendingRoutablePeer { peer, address } => {
                println!("Pending routability of peer: {:?} -> Address: {:?}", peer, address)
                // If the peer is to be unconditionally added to the routing table, then the
                // peer can be added using add_address() method

            },
        }
    }
}

pub async fn build_transport(key_pair: identity::Keypair) -> Result<Boxed<(PeerId, StreamMuxerBox)>, Error> {
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