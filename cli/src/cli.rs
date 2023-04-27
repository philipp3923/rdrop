use clap::{command, Parser};

#[derive(Parser)]
#[command(name = "rdrop")]
#[command(author = "Simon S., Lars Z., Philipp E.")]
#[command(version = "1.0")]
#[command(about = "Tool to send files over a encrypted p2p socket connection.", long_about = None)]
struct Cli {
    /// The pattern to look for
    pattern: String,
    /// The path to the file to read
    path: std::path::PathBuf,
}
