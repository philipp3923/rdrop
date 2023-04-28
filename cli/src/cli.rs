use clap::{command, Parser, Subcommand};
use connection::client::WaitingClient;
use connection::ip::{Ipv4, Ipv6};

#[derive(Parser)]
#[command(name = "rdrop")]
#[command(author = "Simon S., Lars Z., Philipp E.")]
#[command(version = "1.0")]
#[command(about = "Send files over a encrypted p2p socket connection.", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands
}

#[derive(Subcommand)]
enum Commands {

    /// send a file
    Send {
        /// ip address to connect to
        #[arg(short, long, value_name = "IP ADDRESS")]
        ip: String,
        /// ip address to connect to
        #[arg(short, long, value_name = "PORT")]
        port: u16,
    },
    /// receive a file
    Receive {
        /// ip address to connect to
        #[arg(short, long, value_name = "IP ADDRESS")]
        ip: String,
        /// ip address to connect to
        #[arg(short, long, value_name = "PORT")]
        port: u16,
    },
    /// show ip address information
    Show {

    }

}

impl Args {

}

pub(crate) fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Send { .. } => {}
        Commands::Receive { .. } => {}
        Commands::Show { .. } => {
            match WaitingClient::<Ipv4>::new() {
                Ok(client) => {
                    println!("ipv4: {} : {}", client.get_address(), client.get_port());
                }
                Err(_e) => {
                    println!("ipv4: NOT AVAILABLE");
                }
            }

            match WaitingClient::<Ipv6>::new() {
                Ok(client) => {
                    println!("ipv6: {} : {}", client.get_address(), client.get_port());
                }
                Err(_e) => {
                    println!("ipv6: NOT AVAILABLE");
                }
            }

        }
    }
}