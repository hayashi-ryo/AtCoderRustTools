use reqwest::cookie::Jar;
use reqwest::Client;
use rpassword::read_password;
use scraper::{Html, Selector};
use std::{
    error::Error,
    io::{self, Write},
    sync::Arc,
};

pub async fn execute() -> Result<(), Box<dyn Error>> {
    println!("ログイン処理を開始します...");
    let base_url = "https://atcoder.jp";
    let credentials = match get_credentials() {
        Ok(credentials) => credentials,
        Err(e) => return Err(format!("ログイン情報の取得に失敗しました: {}", e).into()),
    };
    match login_to_atcoder(&credentials, base_url).await {
        Ok(_) => {
            println!("AtCoderへのログインが完了しました！");
            Ok(())
        }
        Err(e) => Err(format!("ログイン中にエラーが発生しました: {}", e).into()),
    }
}

pub struct UserCredentials {
    pub user_id: String,
    pub password: String,
}

impl UserCredentials {
    pub fn new(user_id: String, password: String) -> Self {
        UserCredentials { user_id, password }
    }
}

pub fn get_credentials() -> Result<UserCredentials, io::Error> {
    let user_id = prompt_user("User ID: ")?;
    let password = prompt_password("Password: ")?;

    Ok(UserCredentials::new(user_id, password))
}

fn prompt_user(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn prompt_password(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let password = read_password()?;
    Ok(password)
}

pub async fn login_to_atcoder(
    credentials: &UserCredentials,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let cookie_store = Arc::new(Jar::default());
    let login_url = format!("{}/login", base_url);

    let client = Client::builder()
        .cookie_store(true)
        .cookie_provider(Arc::clone(&cookie_store))
        .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
        .build()?;

    // CSRFトークンを取得
    let selector = Selector::parse("input[name=\"csrf_token\"]").unwrap();
    let body = client.get(&login_url).send().await?.text().await?;
    let document = Html::parse_document(&body);
    let csrf_token = document
        .select(&selector)
        .next()
        .unwrap()
        .value()
        .attr("value")
        .unwrap()
        .to_string();

    println!("CSRFトークン: {}", csrf_token);

    let login_form = [
        ("username", credentials.user_id.as_str()),
        ("password", credentials.password.as_str()),
        ("csrf_token", csrf_token.as_str()),
    ];

    let login_response = client.post(&login_url).form(&login_form).send().await?;
    let status = login_response.status();

    if status == reqwest::StatusCode::FOUND {
        if let Some(location) = login_response.headers().get("Location") {
            let location_str = location.to_str()?;
            println!("Locationヘッダー: {}", location_str);
            if location_str == "/home" {
                println!("ログインに成功しました！");
                return Ok(());
            } else {
                return Err(format!(
                    "ログイン失敗: Locationが/homeではありません ({})",
                    location_str
                )
                .into());
            }
        }
        Err("ログイン失敗: Locationヘッダーが見つかりません".into())
    } else {
        Err(format!("ログイン失敗: ステータスコードが想定外です ({})", status).into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use mockito::{Matcher, Server};

    #[tokio::test]
    async fn login_to_atcoder_success() {
        let mut server = Server::new_async().await;
        let base_url = server.url();
        let _get_mock = server
            .mock("GET", "/login")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(
                r#"
                <html>
                    <form>
                        <input type="hidden" name="csrf_token" value="mock_csrf_token">
                    </form>
                </html>
            "#,
            )
            .create();
        let _post_mock = server
            .mock("POST", "/login")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("username=mock_user".to_string()),
                Matcher::Regex("password=mock_password".to_string()),
                Matcher::Regex("csrf_token=mock_csrf_token".to_string()),
            ]))
            .with_status(302) // リダイレクトを模倣
            .with_header("Location", "/home") // リダイレクト先
            .create();

        let credentials =
            UserCredentials::new("mock_user".to_string(), "mock_password".to_string());

        let response = login_to_atcoder(&credentials, &base_url).await;
        assert!(response.is_ok());

        _get_mock.assert();
        _post_mock.assert();
    }

    #[tokio::test]
    async fn login_to_atcoder_failed() {
        let mut server = Server::new_async().await;
        let base_url = server.url();

        let _get_mock = server
            .mock("GET", "/login")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(
                r#"
            <html>
                <form>
                    <input type="hidden" name="csrf_token" value="mock_csrf_token">
                </form>
            </html>
        "#,
            )
            .create();
        let _post_mock = server
            .mock("POST", "/login")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("username=mock_user".to_string()),
                Matcher::Regex("password=mock_password".to_string()),
                Matcher::Regex("csrf_token=mock_csrf_token".to_string()),
            ]))
            .with_status(200) // リダイレクトを模倣
            .with_header("Location", "/home") // リダイレクト先
            .create();
        let credentials =
            UserCredentials::new("mock_user".to_string(), "mock_password".to_string());
        let response = login_to_atcoder(&credentials, &base_url).await;
        assert!(response.is_err());
        // モックが期待通り呼び出されたことを確認
        _get_mock.assert();
        _post_mock.assert();
    }
}
