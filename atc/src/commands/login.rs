use reqwest::cookie::Jar;
use reqwest::{Client, Response, StatusCode};
use rpassword::read_password;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use super::config::{get_session_file, BASE_URL};
const SESSION_EXPIRY: u64 = 86400; // 24時間

/// ログイン処理のエントリーポイント
pub async fn execute() -> Result<(), Box<dyn Error>> {
    let session_path = get_session_file();

    if let Some(session) = Session::load(&session_path)? {
        if !session.is_expired() {
            println!("既存のセッションを利用します");
            return Ok(());
        }
    }

    println!("再ログインを実施します。");
    let credentials =
        get_credentials().map_err(|e| format!("認証情報の取得に失敗しました: {}", e))?;

    let session = login_to_atcoder(&credentials, BASE_URL)
        .await
        .map_err(|e| format!("ログイン中にエラーが発生しました: {}", e))?;

    session.save(&session_path)?;

    println!("新しいセッションを保存しました");

    Ok(())
}

/// ログイン情報の構造体
#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    pub username: String,
    pub csrf_token: String,
    pub session_cookie: String,
    pub last_login_time: u64,
}

impl Session {
    /// セッション情報を保存
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// セッション情報をロード
    pub fn load(path: &Path) -> Result<Option<Self>, io::Error> {
        if !path.exists() {
            return Ok(None);
        }

        let data = fs::read_to_string(path)?;
        match serde_json::from_str(&data) {
            Ok(session) => Ok(Some(session)),
            Err(_) => Ok(None), // 破損した場合は None を返す
        }
    }

    /// 再ログインが必要か判定
    pub fn is_expired(&self) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        current_time - self.last_login_time > SESSION_EXPIRY
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

/// AtCoderにログイン
pub async fn login_to_atcoder(
    credentials: &UserCredentials,
    base_url: &str,
) -> Result<Session, Box<dyn std::error::Error>> {
    let cookie_store = Arc::new(Jar::default());
    let login_url = format!("{}/login", base_url);

    let client = Client::builder()
        .cookie_store(true)
        .cookie_provider(Arc::clone(&cookie_store))
        .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
        .build()?;

    // CSRFトークンを取得
    let csrf_token = get_csrf_token(&client, &login_url).await?;

    // ログイン確認
    let login_form = [
        ("username", credentials.user_id.as_str()),
        ("password", credentials.password.as_str()),
        ("csrf_token", csrf_token.as_str()),
    ];
    let login_response = client.post(&login_url).form(&login_form).send().await?;
    validate_login(&login_response)?;
    println!("{:?}", login_response);
    // セッションCookie
    let session_cookie = extract_cookie(&login_response)?;
    let session = Session {
        username: credentials.user_id.clone(),
        csrf_token,
        session_cookie,
        last_login_time: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    };

    Ok(session)
}

/// CSRFトークンを取得
async fn get_csrf_token(client: &Client, url: &str) -> Result<String, Box<dyn Error>> {
    let selector = Selector::parse("input[name=\"csrf_token\"]").unwrap();
    let body = client.get(url).send().await?.text().await?;
    let document = Html::parse_document(&body);
    let csrf_token = document
        .select(&selector)
        .next()
        .ok_or("CSRFトークンが見つかりませんでした")?
        .value()
        .attr("value")
        .ok_or("CSRFトークンの値が取得できません")?
        .to_string();
    Ok(csrf_token)
}

/// ログイン成功判定
fn validate_login(response: &Response) -> Result<(), Box<dyn Error>> {
    if response.status() == StatusCode::FOUND {
        if let Some(location) = response.headers().get("Location") {
            if location.to_str()? == "/home" {
                return Ok(());
            }
        }
    }
    Err("ログインに失敗しました。IDまたはパスワードを確認してください。".into())
}

/// Cookieを取得
fn extract_cookie(response: &Response) -> Result<String, Box<dyn Error>> {
    let mut cookie_string = String::new();

    for header_value in response.headers().get_all("set-cookie").iter() {
        let cookie = header_value.to_str()?;
        if cookie.starts_with("REVEL_SESSION=") {
            if !cookie_string.is_empty() {
                cookie_string.push_str("; ");
            }
            let parts: Vec<&str> = cookie.split("; ").collect();
            cookie_string.push_str(parts[0]); // `Set-Cookie` には属性が含まれているので `key=value` の部分だけ取得
        }
    }

    if cookie_string.is_empty() {
        return Err("セッションCookie (REVEL_SESSION) が取得できませんでした。".into());
    }

    Ok(cookie_string)
}

#[cfg(test)]
mod test {
    use super::*;
    use mockito::{Matcher, Server};
    use tempfile;

    #[tokio::test]
    async fn test_login_to_atcoder_success() {
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
            .with_header(
                "Set-Cookie",
                "session_cookie=mock_session_cookie; Path=/; HttpOnly",
            )
            .create();

        let credentials =
            UserCredentials::new("mock_user".to_string(), "mock_password".to_string());

        let response = login_to_atcoder(&credentials, &base_url).await;
        assert!(response.is_ok());

        let session = response.unwrap();
        assert_eq!(session.username, "mock_user");
        assert_eq!(session.csrf_token, "mock_csrf_token");
        assert_eq!(session.session_cookie, "session_cookie=mock_session_cookie");

        _get_mock.assert();
        _post_mock.assert();
    }

    #[tokio::test]
    async fn test_login_to_atcoder_failed() {
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

    #[test]
    fn test_save_and_load_session() {
        let work_dir = tempfile::tempdir().expect("");
        let session = Session {
            username: "mock_user".to_string(),
            csrf_token: "mock_csrf_token".to_string(),
            session_cookie: "mock_session_cookie".to_string(),
            last_login_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        let session_file_path = work_dir.path().join("session.json");
        let test_path = session_file_path.to_str().unwrap();
        //        save_session(&session, test_path).unwrap();

        let saved_data = fs::read_to_string(test_path).unwrap();
        let expected_json = serde_json::to_string_pretty(&session).unwrap();
        assert_eq!(
            saved_data, expected_json,
            "保存されたJSONデータが想定と異なる"
        );

        //      let loaded_session = load_session(test_path).unwrap();
        //assert_eq!(session.username, loaded_session.username);
        //assert_eq!(session.csrf_token, loaded_session.csrf_token);
        //assert_eq!(session.session_cookie, loaded_session.session_cookie);
    }

    #[test]
    fn test_is_relogin_required() {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let valid_session = Session {
            username: "mock_user".to_string(),
            csrf_token: "mock_csrf_token".to_string(),
            session_cookie: "mock_session_cookie".to_string(),
            last_login_time: current_time - 1000, // 1000秒前 → まだ有効
        };
        // assert!(!is_relogin_required(&valid_session));

        let expired_session = Session {
            username: "mock_user".to_string(),
            csrf_token: "mock_csrf_token".to_string(),
            session_cookie: "mock_session_cookie".to_string(),
            last_login_time: current_time - (SESSION_EXPIRY + 1), // 期限切れ
        };
        //       assert!(is_relogin_required(&expired_session));
    }

    #[tokio::test]
    async fn test_get_csrf_token() {
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
                        <input type="hidden" name="csrf_token" value="test_csrf_token">
                    </form>
                </html>
            "#,
            )
            .create();

        let client = Client::new();
        let csrf_token = get_csrf_token(&client, &format!("{}/login", base_url))
            .await
            .unwrap();

        assert_eq!(csrf_token, "test_csrf_token");
        _get_mock.assert();
    }
}
