use std::{
    error::Error,
    collections::HashMap,
};
use tokio::{
    sync::broadcast,
    io::{self, AsyncBufReadExt},
};
use libp2p::{
    identity, mdns, mplex, noise, tcp, Multiaddr, PeerId, Transport,
    core::upgrade,
    futures::StreamExt,
    floodsub::{self, Floodsub, FloodsubEvent},
    swarm::{NetworkBehaviour, SwarmEvent},
};
use snowflake::SnowflakeIdGenerator;
lazy_static::lazy_static!(
    static ref CHAN_CAP: usize = 100;
);
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "MyBehaviourEvent")]
struct MyBehaviour {
    floodsub: Floodsub,
    mdns: mdns::tokio::Behaviour,
}
#[allow(clippy::large_enum_variant)]
enum MyBehaviourEvent {
    Floodsub(FloodsubEvent),
    Mdns(mdns::Event),
}
impl From<FloodsubEvent> for MyBehaviourEvent {
    fn from(event: FloodsubEvent) -> Self {
        MyBehaviourEvent::Floodsub(event)
    }
}
impl From<mdns::Event> for MyBehaviourEvent {
    fn from(event: mdns::Event) -> Self {
        MyBehaviourEvent::Mdns(event)
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let keys = identity::Keypair::generate_ed25519();
    let id = PeerId::from(keys.public());
    let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseAuthenticated::xx(&keys)?)
        .multiplex(mplex::MplexConfig::new())
    .boxed();
    let topic = floodsub::Topic::new("0");
    let mdns_behaviour = mdns::Behaviour::new(mdns::Config::default())?;
    let mut behaviour = MyBehaviour {
        floodsub: Floodsub::new(id),
        mdns: mdns_behaviour,
    };
    behaviour.floodsub.subscribe(topic.clone());
    let mut swarm = libp2p::swarm::Swarm::with_tokio_executor(transport, behaviour, id);
    if let Some(to_dial) = std::env::args().nth(1) {
        let addr: Multiaddr = to_dial.parse()?;
        swarm.dial(addr)?;
        println!("init-> dialed {to_dial:?}");
    }
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    let (tx, mut rx) = broadcast::channel::<String>(*CHAN_CAP);
    let internal_tx = tx.clone();
    let mut internal_rx = rx.resubscribe();
    tokio::spawn(async move {
        let mut stdin = io::BufReader::new(io::stdin()).lines();
        loop {
            tokio::select!{
                line = stdin.next_line() => {
                    match line {
                        Err(err) => eprintln!("console-> failed to parse stdin: {err}"),
                        Ok(line) => {
                            match line {
                                None => eprintln!("console-> stdin closed"),
                                Some(line) => {
                                    match internal_tx.send(line) {
                                        Err(err) => eprintln!("console-> failed to send stdin to swarm: {err}"),
                                        Ok(_) => {}
                                    }
                                }
                            }
                        }
                    };
                }
                recv = internal_rx.recv() => {
                    match recv {
                        Err(err) => eprintln!("internal-> failed to receive message: {err}"),
                        Ok(msg) => {
                            let cmds = msg.split(" ").collect::<Vec<&str>>();
                            match cmds[0] {
                                "" => eprintln!("internal-> malformed message, first command blank: {msg}"),
                                _ => {},
                            }
                        }
                    }
                }
            }
        }
    });
    let cache: HashMap<String, String> = HashMap::new();
    let mut id_gen = SnowflakeIdGenerator::new(1, 1);
    loop {
        tokio::select!{
            recv = rx.recv() => {
                match recv {
                    Err(err) => eprintln!("swarm-> failed to receive message: {err}"),
                    Ok(msg) => {
                        let cmds = msg.split(" ").collect::<Vec<&str>>();
                        match cmds[0] {
                            "swarm" => swarm.behaviour_mut().floodsub.publish(topic.clone(), cmds[1..].join(" ").as_bytes()),
                            "cache_get" => {
                                match cache.get(cmds[2]) {
                                    None => eprintln!("swarm-> requested cache key has no value: {msg}"),
                                    Some(val) => {
                                        match tx.send(["cache_return", cmds[1], val].join(" ")) {
                                            Err(err) => eprintln!("swarm: failed to send response to cache request: {err}"),
                                            Ok(_) => {}
                                        }
                                    }
                                }
                            },
                            "id" => println!("swarm-> local peer id: {}", swarm.local_peer_id()),
                            "random" => println!("swarm-> generated random: {}", id_gen.generate()),
                            "" => eprintln!("swarm-> malformed message, first command blank: {msg}"),
                            _ => {},
                        }
                    }
                }
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("swarm-> listening to {address:?}");
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Floodsub(FloodsubEvent::Message(message))) => {
                        println!(
                                "swarm-> received from {:?}: {:?}",
                                message.source,
                                String::from_utf8_lossy(&message.data),
                            );
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(event)) => {
                        match event {
                            mdns::Event::Discovered(list) => {
                                for (peer, _) in list {
                                    swarm.behaviour_mut().floodsub.add_node_to_partial_view(peer);
                                }
                            }
                            mdns::Event::Expired(list) => {
                                for (peer, _) in list {
                                    if !swarm.behaviour().mdns.has_node(&peer) {
                                        swarm.behaviour_mut().floodsub.remove_node_from_partial_view(&peer);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}