use lazy_static::lazy_static;

lazy_static!{
    static ref ARGS: Vec<String> = std::env::args().collect();
    static ref SERVER: String = ARGS[1].clone();
    static ref PORT: String = ARGS[2].clone();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rcl = redis::Client::open(format!("redis://{}:{}", *SERVER, *PORT))?;
    let mut rc = rcl.get_connection()?;
    let ping = redis::cmd("ping").query::<String>(&mut rc)?;
    println!("we say ping, server says: {}", redis::cmd("ping").query::<String>(&mut rc)?);
    if ping != "PONG" { panic!("server did not pong..."); }
    
    Ok(())
}
