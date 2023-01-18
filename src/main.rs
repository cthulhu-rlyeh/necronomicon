use std::error::Error;
use libp2p::futures::StreamExt;
use serde_derive::{Serialize, Deserialize};
use tokio::time::{sleep, Duration};
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{
    record::Key, AddProviderOk, Kademlia, KademliaEvent, PeerRecord,
    PutRecordOk, QueryResult, Quorum, Record,
};
use libp2p::{
    tokio_development_transport, identity, mdns, PeerId, Swarm,
    swarm::{NetworkBehaviour, SwarmEvent},
};
use libp2p_kad::{GetProvidersOk, GetRecordOk};
use rand::prelude::*;

lazy_static::lazy_static!{
    static ref ARGS: Vec<String> = std::env::args().collect();
    static ref TEMPO: u64 = 37;
    static ref SHORT_TICKS: i32 = 10;
    static ref LONG_TICKS: i32 = 100;
}

#[derive(Serialize,Deserialize)]
struct DeviceData {
    mac: String,
    ip: String,
    name: String,
    os: String,
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "NetworkEvent")]
struct Network {
    kademlia: Kademlia<MemoryStore>,
    mdns: mdns::async_io::Behaviour,
}
#[allow(clippy::large_enum_variant)]
enum NetworkEvent {
    Kademlia(KademliaEvent),
    Mdns(mdns::Event),
}
impl From<KademliaEvent> for NetworkEvent {
    fn from(event: KademliaEvent) -> Self {
        NetworkEvent::Kademlia(event)
    }
}
impl From<mdns::Event> for NetworkEvent {
    fn from(event: mdns::Event) -> Self {
        NetworkEvent::Mdns(event)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error+Send+Sync>> {
    let mut console = false;
    if ARGS.len() > 0 {
        if ARGS[0] == "console" {
            console = true;
        }
    }

    let mut rng = rand::rngs::OsRng;

    tokio::spawn(swarm_handler());
    
    let mut short_ticker = *SHORT_TICKS;
    let mut long_ticker = *LONG_TICKS;
    loop {
        // todo: add fastest actions here
        // if necromancer
            // todo: if too many necromancers are in the swarm, demote
        // else
            // todo: if no necromancer is found in the swarm, promote
        // end if

        if rng.gen::<bool>() {
            short_ticker += 1;
            if short_ticker > *SHORT_TICKS {
                // todo: add slower actions here
                // todo: collect system data and store in swarm
                // todo: check swarm for zombie tasks and run accordingly
                // if necromancer
                    // todo: sync swarm tasks with graveyard
                    // todo: upload current swarm data to graveyard
                    // todo: if graveyard unreachable, demote
                // end if

                println!("-> beep");
                short_ticker = 0;
            }
        }

        if rng.gen::<bool>() {
            long_ticker += 1;
            if long_ticker > *LONG_TICKS {
                // todo: add slowest actions here
                // if necromancer
                    // todo: sweep for lonely systems without a zombie
                    // todo: spawn zombies on lonely systems
                // end if
                println!("---> boop");
                long_ticker = 0;
            }
        }

        sleep(Duration::from_secs(*TEMPO)).await;
    }
}

async fn swarm_handler() -> Result<(), Box<dyn Error+Send+Sync>> {
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    let transport = tokio_development_transport(local_key)?;

    let mut swarm = {
        let store = MemoryStore::new(local_peer_id);
        let kademlia = Kademlia::new(local_peer_id, store);
        let mdns = mdns::async_io::Behaviour::new(mdns::Config::default())?;
        let network = Network { kademlia, mdns };
        Swarm::with_tokio_executor(transport, network, local_peer_id)
    };

    swarm.listen_on("/ipv4/0.0.0.0/tcp/0".parse()?)?;
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("listening on {address:?}");
            },
            SwarmEvent::Behaviour(NetworkEvent::Mdns(mdns::Event::Discovered(list))) => {
                for (peer_id, multiaddr) in list {
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                }
            }
            SwarmEvent::Behaviour(NetworkEvent::Kademlia(KademliaEvent::OutboundQueryProgressed { result, ..})) => {
                match result {
                    QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders { key, providers, .. })) => {
                        for peer in providers {
                            println!(
                                "peer {peer:?} provides key {:?}",
                                std::str::from_utf8(key.as_ref()).unwrap()
                            );
                        }
                    }
                    QueryResult::GetProviders(Err(err)) => {
                        eprintln!("failed to get providers: {err:?}");
                    }
                    QueryResult::GetRecord(Ok(
                        GetRecordOk::FoundRecord(PeerRecord {
                            record: Record { key, value, .. },
                            ..
                        })
                    )) => {
                        println!(
                            "got record {:?} {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap(),
                            std::str::from_utf8(&value).unwrap(),
                        );
                    }
                    QueryResult::GetRecord(Ok(_)) => {}
                    QueryResult::GetRecord(Err(err)) => {
                        eprintln!("failed to get record: {err:?}");
                    }
                    QueryResult::PutRecord(Ok(PutRecordOk { key })) => {
                        println!(
                            "successfully put record {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap()
                        );
                    }
                    QueryResult::PutRecord(Err(err)) => {
                        eprintln!("failed to put record: {err:?}");
                    }
                    QueryResult::StartProviding(Ok(AddProviderOk { key })) => {
                        println!(
                            "successfully put provider record {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap()
                        );
                    }
                    QueryResult::StartProviding(Err(err)) => {
                        eprintln!("failed to put provider record: {err:?}");
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn _collector() -> String {
    let os = os_info::get();
    let data = DeviceData {
        mac: match mac_address::get_mac_address() {
            Err(_) => "".to_owned(),
            Ok(ok) => match ok {
                None => "".to_owned(),
                Some(ok) => ok.to_string(),
            },
        },
        ip: match local_ip_address::local_ip() {
            Err(_) => "".to_owned(),
            Ok(ok) => ok.to_string(),
        },
        name: match hostname::get() {
            Err(_) => "".to_owned(),
            Ok(ok) => ok.to_string_lossy().to_string(),
        },
        os: [os.os_type().to_string(), os.version().to_string(), os.bitness().to_string()].join(" "),
    };
    match serde_json::to_string(&data) {
        Err(_) => "".to_owned(),
        Ok(ok) => ok,
    }
}
