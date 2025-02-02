mod commands;
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
    let cli = Cli::parse();
    match cli.command {
        Commands::Login => {
            if let Err(e) = commands::login::execute().await {
                eprintln!("Error: {}", e);
            }
        }
        Commands::Test { problem_name } => {
            if let Err(e) = commands::test::execute(&problem_name) {
                eprintln!("Error: {}", e);
            }
        }
        Commands::Download { contest_name } => {
            println!("DEBUG0");
            if let Err(e) = commands::download::execute(&contest_name).await {
                eprintln!("Error: {}", e);
            }
        }
    }
}
/*
#[cfg(test)]
mod test {
    use assert_cmd::Command;
    use mockall::mock;

    #[test]
    fn test_login_subcommmand() {
        let mut cmd = Command::cargo_bin("cargo-atc").unwrap();
        cmd.arg("login")
            .write_stdin("\n") // 標準入力を空にする
            .assert()
            .success();
    }

    #[tokio::test]
    async fn test_login_is_called() {
        mock! {
            pub login {
                pub async fn execute() -> Result<(), Box<dyn std::error::Error>>;
            }
        }
        let mut cmd = Command::cargo_bin("cargo-atc").unwrap();
        let result = cmd.arg("login").assert();

        result.success();
    }
}
 */
