use clap::Parser;
use rituals::Ritual;
use std::fs;

/// A simple program to run rituals
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The path to the ritual file
    #[arg(short, long)]
    ritual: String,
}

fn main() {
    let args = Args::parse();

    let ritual_file = fs::read_to_string(args.ritual).expect("Unable to read ritual file");
    let ritual: Ritual = serde_yaml::from_str(&ritual_file).expect("Unable to parse ritual file");

    println!("Running ritual: {}", ritual.name);
    for step in &ritual.steps {
        println!("  - {}", step);
    }
}