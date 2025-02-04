mod commands;
use std::env;

use clap::{Parser, Subcommand};
use tokio;

#[derive(Parser)]
#[command(name = "cargo-atc")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Login,
    Test { problem_name: String },
    Download { contest_name: String },
}

#[tokio::main]
async fn main() {
    let work_dir = env::current_dir().expect("Failed to get current directory");

    let cli = Cli::parse();
    match cli.command {
        Commands::Login => {
            if let Err(e) = commands::login::execute().await {
                eprintln!("Error: {}", e);
            }
        }
        Commands::Test { problem_name } => {
            if let Err(e) = commands::test::execute(&work_dir, &problem_name) {
                eprintln!("Error: {}", e);
            }
        }
        Commands::Download { contest_name } => {
            println!("DEBUG0");
            if let Err(e) = commands::download::execute(&work_dir, &contest_name).await {
                eprintln!("Error: {}", e);
            }
        }
    }
}
