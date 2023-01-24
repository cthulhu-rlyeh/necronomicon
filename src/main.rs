use std::{
    error::Error,
    collections::HashMap,
};
use tokio::io::{self, AsyncBufReadExt};
use libp2p::{
    identity, mdns, mplex, noise, tcp, Multiaddr, PeerId, Transport,
    core::upgrade,
    futures::StreamExt,
    floodsub::{self, Floodsub, FloodsubEvent},
    swarm::{NetworkBehaviour, SwarmEvent},
};
lazy_static::lazy_static!(

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

    let topic = floodsub::Topic::new("chat");

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
        println!("dialed {to_dial:?}");
    }

    let mut stdin = io::BufReader::new(io::stdin()).lines();

    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let cache: HashMap<u64, String> = HashMap::new();
    loop {
        tokio::select!{
            line = stdin.next_line() => {
                let line = line?.expect("stdin closed");
                swarm.behaviour_mut().floodsub.publish(topic.clone(), line.as_bytes());
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("listening to {address:?}");
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Floodsub(FloodsubEvent::Message(message))) => {
                        println!(
                                "received from {:?}: {:?}",
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