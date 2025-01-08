mod login;

fn main() {
    println!("ログイン処理を開始します...");
    match login::get_credentials() {
        Ok(credentials) => {
            // 入力結果の確認（デバッグ用、実運用では削除）
            println!("ユーザーID: {}", credentials.user_id);
            println!("パスワードは入力されました（表示されません）。");

            // 次の処理（例: AtCoderへのログイン）を呼び出す
            // login::login_to_atcoder(&credentials); // 次に実装
        }
        Err(e) => {
            eprintln!("ログイン情報の取得に失敗しました: {}", e);
        }
    }
}
