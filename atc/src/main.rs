mod commands;
use clap::{Args, Parser, Subcommand};
use tokio;

#[derive(Parser)]
#[command(name = "cargo-atc")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    login,
    test,
}

#[tokio::main]
async fn main() {
    println!("ログイン処理を開始します...");
    let login_url = "https://atcoder.jp";
    match commands::login::get_credentials() {
        Ok(credentials) => {
            match commands::login::login_to_atcoder(&credentials, &login_url).await {
                Ok(_) => println!("AtCoderへのログインが完了しました！"),
                Err(e) => eprintln!("ログイン中にエラーが発生しました: {}", e),
            }
        }
        Err(e) => {
            eprintln!("ログイン情報の取得に失敗しました: {}", e);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
}
