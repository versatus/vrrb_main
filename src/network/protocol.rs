use env_logger::{Builder, Env};
use libp2p::{
    core::{
        either::EitherTransport,
        muxing::StreamMuxerBox,
        transport, 
        transport::upgrade::Version,
    },
    kad::record::store::MemoryStore;
    kad::{
        AddProviderOk,
        Kademlia,
        KademliaEvent,
        PeerRecord,
        PutRecordOk,
        QueryResult,
        Quorum,
        Record,
        record::Key,
    }
    swarm::{
        NetworkBehaviourEventProcess,
        SwarmEvent
    }, 
    gossipsub::{
        self,
        Gossipsub, 
        MessageAuthenticity, 
        GossipsubConfig,
        IdentTopic,
        GossipsubEvent,
    },
    identify::{
        Identify, 
        IdentifyInfo, 
        IdentifyConfig, 
        IdentifyEvent
    },
    identity,
    multiaddr::Protocol,
    noise,
    ping::{
        self, 
        Ping, 
        PingConfig, 
        PingEvent,
    },
    pnet::{
        PnetConfig,
        PreSharedKey,
    },
    tcp::TcpConfig,
    yamux::YamuxConfig,
    Multiaddr, 
    NetworkBehaviour, 
    PeerId, 
    Swarm, 
    Transport,
};
use std::{
    env,
    error::Error,
    fs,
    path::Path,
    str::FromStr,
    task::{Context, Poll},
    time::Duration
};
use futures::executor::block_on;
use futures::prelude::*;

struct VrrbNetworkBehavior {
    gossipsub: Gossipsub,
    identify: Identify,
    kademlia: Kademlia,
    ping: Ping,

}

impl NetworkBehaviourEventProcess<IdentifyEvent> for VrrbNetworkBehavior {
    // called when 'identify'
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Received {
                peer_id,
                info,
            } => {},
            IdentifyEvent::Sent {
                peer_id
            } => {},
            IdentifyEvent::Pushed {
                peer_id
            } => {},
            IdentifyEvent::Error {
                peer_id,
                error,
            } => {}
        }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, event: GossipsubEvent) {
        match event {
            GossipsubEvent::Message {
                propagation_source: peer_id,
                message_id: id,
                message
            } =>{ 
                
                println!("Got message: {}, with id: {} from peer: {:?}",
                String::from_utf8_lossy(&message.data),
                id,
                peer_id);
                // check message headers for channel match
                //
                // foreward the message for processing
                //
                // If the message is a new txn, new block, new claim homesteading/acquisition
                // send to validator
                //
                // if the message is a validator send to vpu
                //
                // if the message is a confirmation of a txn, block, claim homesteading/acquisition
                // for txn: add to mineable
                // for block: confirm network state through consensus vote in governance channel
                // for claim homesteading/acquisition update local state, etc.
            },
            GossipsubEvent::Subscribed => {},
            GossipsubEvent::Unsubscribed => {},
        }
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, event: PingEvent) {
        use ping::handler::{PingFailure, PingSuccess};
        match event {
            PingEvent {
                peer,
                result: Result::Ok(PingSuccess::Ping { rtt }),
            } => {
                println!(
                    "ping: rtt to {} is {} ms",
                    peer.to_base58(),
                    rtt.as_millis()
                );
            },
            PingEvent {
                peer,
                result: Result::Ok(PingSuccess::pong),
            } => {
                // In the event of a successful ping with a returned pong
                // maintain the peer
                println!("ping: pong from {}", peer.to_base58());
            },
            PingEvent {
                peer,
                result: Result::Err(PingFailure::Timeout),
            } => {
                // In the event of a ping failure, propagate the removal of the peer
                println!("ping: timeout to {}", peer.to_base58());
            },
            PingEvent {
                peer,
                result: Result::Err(PingFailure::Other { error }),
            } => {
                // In the event of a ping failure, propagate the removal of the peer
                println!("ping: failure with {}: {}", peer.to_base58(), error);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, message: KademliaEvent) {
        match message {
            KademliaEvent::QueryResult { id, result, stats } => {
                println!("Received query result:\n id: {:?} \n result: {:?}, stats: {:?}", &id, &result, &stats);
                match result {
                    _ => {}
                }
            },
            KademliaEvent::RoutingUpdated { peer_id, address, old_peer } => {},
            KademliaEvent::UnroutablePeer { peer_id } => {},
            KademliaEvent::RoutablePeer { peer_id, address } => {},
            KademliaEvent::PendingRoutablePeer { peer_id, addr } => {},
        }
    }
}

impl NetworkBehaviourEventProcess<SwarmEvent> for VrrbNetworkBehavior {
    fn inject_event(&mut self, event: SwarmEvent) {
        match event {
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
            } => {},
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                num_established,
                cause
            } => {},
            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
            } => {},
            SwarmEvent::IncomingConnectionError {
                local_addr,
                send_back_addr,
                error,
            } => {},
            SwarmEvent::BannedPeer {
                peer_id,
                endpoint
            } => {},
            SwarmEvent::UnreachableAddr {
                peer_id,
                address,
                error,
                attempts_remaining,
            },
            SwarmEvent::UnknownPeerUnreachableAddr {
                address,
                error,
            },
            SwarmEvent::NewListenAddr(address) => {},
            SwarmEvent::ExpiredListenAddr(address) => {},
            SwarmEvent::ListenerClosed {
                addresses,
                reason,
            } => {},
            SwarmEvent::ListenerError {
                error
            } => {},
            SwarmEvent::Dialing(peer_id) => {}
        }
    }
}