mod login;
use tokio;

#[tokio::main]
async fn main() {
    println!("ログイン処理を開始します...");
    match login::get_credentials() {
        Ok(credentials) => match login::login_to_atcoder(&credentials).await {
            Ok(_) => println!("AtCoderへのログインが完了しました！"),
            Err(e) => eprintln!("ログイン中にエラーが発生しました: {}", e),
        },
        Err(e) => {
            eprintln!("ログイン情報の取得に失敗しました: {}", e);
        }
    }
}
