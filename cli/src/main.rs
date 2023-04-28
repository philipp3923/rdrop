use std::string::FromUtf8Error;
use clap::{command, Parser, Subcommand};
use connection::client;
use connection::client::{ClientReader, ClientWriter, WaitingClient};
use connection::ip::{Address, Ipv4, Ipv6};

#[derive(Parser)]
#[command(name = "rdrop")]
#[command(author = "Simon S., Lars Z., Philipp E.")]
#[command(version = "1.0")]
#[command(about = "Send files over a encrypted p2p socket connection.", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// send a file
    Send {
        /// ip address to connect to
        #[arg(short, long, value_name = "IP ADDRESS")]
        ip: String,
        /// port to bind socket to
        #[arg(short, long, value_name = "BIND PORT")]
        bind_port: Option<u16>,
        /// ip address to connect to
        #[arg(short, long, value_name = "PORT")]
        port: u16,
        /// message to send
        #[arg(short, long, value_name = "MESSAGE")]
        msg: String,
    },
    /// receive a file
    Receive {
        /// ip address to connect to
        #[arg(short, long, value_name = "IP ADDRESS")]
        ip: String,
        /// port to bind socket to
        #[arg(short, long, value_name = "BIND PORT")]
        bind_port: Option<u16>,
        /// ip address to connect to
        #[arg(short, long, value_name = "PORT")]
        port: u16,
    },
    /// show ip address information
    Show {},
}

fn send_msg<A: Address>(ip: A, port: u16, bind_port: Option<u16>, msg: String) {
    let (_, mut writer) = match connect(ip, port, bind_port) {
        Ok(rw) => rw,
        Err(e) => {println!("{}",e); return;}
    };

    println!("Connected.");

    writer.write(msg.as_bytes());

    println!("Message sent.");
}

fn read_msg<A: Address>(ip: A, port: u16, bind_port: Option<u16>) {
    let (mut reader, _) = match connect(ip, port, bind_port) {
        Ok(rw) => rw,
        Err(e) => {println!("{}",e); return;}
    };

    println!("Connected.");

    let message = match String::from_utf8(reader.read()) {
        Ok(m) => m,
        Err(_) => {println!("Decoding message failed."); return;}
    };

    println!("Received:\n{}", message);
}

fn connect<A: Address>(ip: A, port: u16, bind_port: Option<u16>) -> Result<(ClientReader, ClientWriter), String> {
    let client = match bind_port {
        Some(p) => WaitingClient::<A>::with_port(p),
        None => WaitingClient::<A>::new(),
    };

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            return Err(format!("Creating client failed: {}", e));
        }
    };

    return match client.connect(ip, port) {
        Ok(c) => Ok(c),
        Err(_) => Err(format!("Connecting failed")),
    };
}

impl Args {}

pub(crate) fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Send { ip, bind_port, port, msg } => {
            match (Ipv4::from_str(&ip), Ipv6::from_str(&ip)) {
                (Ok(ip), _ ) => send_msg::<Ipv4>(ip, port, bind_port, msg),
                (_, Ok(ip)) => send_msg::<Ipv6>(ip, port, bind_port, msg),
                (_, _ ) => println!("given ip address is invalid"),
            }
        }
        Commands::Receive { ip, bind_port, port } => {
            match (Ipv4::from_str(&ip), Ipv6::from_str(&ip)) {
                (Ok(ip), _ ) => read_msg::<Ipv4>(ip, port, bind_port),
                (_, Ok(ip)) => read_msg::<Ipv6>(ip, port, bind_port),
                (_, _ ) => println!("given ip address is invalid"),
            }
        }
        Commands::Show { .. } => {
            match WaitingClient::<Ipv4>::new() {
                Ok(client) => println!("port: {}", client.get_port()),
                Err(_) => {
                    println!("no port available");
                    return;
                }
            }

            match Ipv4::from_public() {
                Ok(address) => {
                    println!("ipv4: {}", address);
                }
                _ => {
                    println!("ipv4: NOT AVAILABLE");
                }
            }

            match Ipv6::from_public() {
                Ok(address) => {
                    println!("ipv6: {}", address);
                }
                _ => {
                    println!("ipv6: NOT AVAILABLE");
                }
            }
        }
    }
}