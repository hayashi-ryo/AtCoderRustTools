use reqwest::{cookie::Jar, Client, Response, StatusCode};
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
            return Ok(());
        }
    }

    println!("login:");
    let credentials =
        get_credentials().map_err(|e| format!("認証情報の取得に失敗しました: {}", e))?;
    let session = login_to_atcoder(&credentials, BASE_URL)
        .await
        .map_err(|e| format!("ログイン中にエラーが発生しました: {}", e))?;
    session.save(&session_path)?;

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
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?; // 親ディレクトリを作成
            }
        }
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

    /// セッションの有効期限が切れているかを判定する。
    ///
    /// - `SESSION_EXPIRY` を超えている場合`true`を返す。
    pub fn is_expired(&self) -> bool {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        current_time - self.last_login_time > SESSION_EXPIRY
    }
}

/// ユーザーIDとパスワードを受け取る構造体
pub struct UserCredentials {
    pub user_id: String,
    pub password: String,
}

/// ユーザー認証情報を作成する
impl UserCredentials {
    pub fn new(user_id: String, password: String) -> Self {
        UserCredentials { user_id, password }
    }
}

/// ユーザーの認証情報を取得する
///
/// - 標準入力からユーザーIDとパスワードを取得する。
/// - パスワードは `rpassword::read_password()` を使用して非表示入力する。
/// - 入力後、`UserCredentials` 構造体として返す。
///
/// # 戻り値
/// - `Ok(UserCredentials)`: ユーザーIDとパスワードの取得に成功した場合
/// - `Err(io::Error)`: 入力の読み取りに失敗した場合
pub fn get_credentials() -> Result<UserCredentials, io::Error> {
    let user_id = prompt_user("User ID: ")?;
    let password = prompt_password("Password: ")?;

    Ok(UserCredentials::new(user_id, password))
}

/// ユーザーにプロンプトを表示し、標準入力から文字列を取得する
///
/// # 引数
/// - `prompt`: ユーザーに表示するプロンプト文字列
///
/// # 戻り値
/// - `Ok(String)`: 入力された文字列（前後の空白は除去）
/// — `Err(io::Error)`: 入力の読み取りに失敗した場合
fn prompt_user(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// ユーザーにプロンプトを表示し、標準入力からパスワードを取得する（非表示入力）
///
/// # 引数
/// - `prompt`: ユーザーに表示するプロンプト文字列
///
/// # 戻り値
/// - `Ok(String)`: 入力されたパスワード（前後の空白は除去）
/// — `Err(io::Error)`: 入力の読み取りに失敗した場合
fn prompt_password(prompt: &str) -> Result<String, io::Error> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let password = read_password()?;
    Ok(password)
}

/// AtCoderにログインし、セッション情報を取得する
///
/// # 引数
/// - `credentials`: ユーザーIDとパスワードを格納した `UserCredentials` 構造体
/// - `base_url`: AtCoderのベースURL (`https://atcoder.jp`)
///
/// # 戻り値
/// - `Ok(Session)`: ログインに成功し、取得したセッション情報を格納した `Session` 構造体
/// - `Err(Box<dyn std::error::Error>)`: ログイン処理が失敗した場合のエラー
///
/// # 例外
/// - AtCoderのページ構造が変更された場合、CSRFトークンの取得に失敗する可能性がある
/// - ログイン失敗時にはエラーメッセージを返す
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

/// CSRFトークンを取得する
///
/// # 引数
/// - `client`: `reqwest::Client` オブジェクト
/// - `url`: トークンを取得するページURL
///
/// # 戻り値
/// - `Ok(String)`: 取得したCSRFトークン
/// - `Err(Box<dyn Error>)`: CSRFトークンの取得に失敗した場合のエラー
///
/// # 例外
/// - AtCoderのページ構造が変更された場合、CSRFトークンの取得に失敗する可能性がある
/// - ネットワークエラーによりページが取得できない場合はエラーを返す
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

/// AtCoderのログインレスポンスを検証し、ログイン成功かどうかを判定する
///
/// # 引数
/// - `response`: `reqwest::Response` オブジェクト（ログインリクエストのレスポンス）
///
/// # 戻り値
/// - `Ok(())`: ログイン成功
/// - `Err(Box<dyn Error>)`: ログインに失敗した場合のエラー
///
/// # 例外
/// - HTTP ステータスコードが 302 でなく、リダイレクトURLが `/home` でない場合はエラーを返す
/// - ネットワークエラーなどが発生した場合、エラーを返す
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

/// AtCoderのログインレスポンスからセッションCookie (`REVEL_SESSION`) を抽出する
///
/// # 引数
/// - `response`: `reqwest::Response` オブジェクト（ログインリクエストのレスポンス）
///
/// # 戻り値
/// - `Ok(String)`: 抽出した `REVEL_SESSION` の値
/// - `Err(Box<dyn Error>)`: セッションCookieの取得に失敗した場合のエラー
///
/// # 例外
/// - `Set-Cookie` ヘッダーに `REVEL_SESSION` が存在しない場合はエラーを返す
/// - `Set-Cookie` に `REVEL_SESSION` 以外の値が含まれている場合は無視し、`REVEL_SESSION` のみを取得する
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
            .with_header("set-cookie", "REVEL_SESSION=mock_session_cookie;")
            .create();

        let credentials =
            UserCredentials::new("mock_user".to_string(), "mock_password".to_string());

        let response = login_to_atcoder(&credentials, &base_url).await;
        assert!(response.is_ok());

        let session = response.unwrap();
        assert_eq!(session.username, "mock_user");
        assert_eq!(session.csrf_token, "mock_csrf_token");
        assert_eq!(session.session_cookie, "REVEL_SESSION=mock_session_cookie;");

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
    fn test_save_and_load_session_success() {
        let work_dir = tempfile::tempdir().expect("");
        let session_file_path = work_dir.path().join("test/session.json");
        let session = Session {
            username: "mock_user".to_string(),
            csrf_token: "mock_csrf_token".to_string(),
            session_cookie: "mock_session_cookie".to_string(),
            last_login_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        session.save(&session_file_path).expect("");
        let saved_data = fs::read_to_string(&session_file_path).unwrap();
        let expected_json = serde_json::to_string_pretty(&session).unwrap();
        assert_eq!(saved_data, expected_json);
        let loaded_session = Session::load(&session_file_path).unwrap().unwrap();
        assert_eq!(session.username, loaded_session.username);
        assert_eq!(session.csrf_token, loaded_session.csrf_token);
        assert_eq!(session.session_cookie, loaded_session.session_cookie);
    }

    #[test]
    fn test_load_invalid_session_file() {
        let work_dir = tempfile::tempdir().expect("");
        let session_file_path = work_dir.path().join("invalid_session.json");
        fs::write(&session_file_path, "{ invalid json }")
            .expect("不正なJSONデータの書き込みに失敗");
        let result = Session::load(&session_file_path).expect("セッションロードに失敗");
        assert!(result.is_none(),);
    }

    #[test]
    fn test_is_expired() {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let valid_session = Session {
            username: "mock_user".to_string(),
            csrf_token: "mock_csrf_token".to_string(),
            session_cookie: "mock_session_cookie".to_string(),
            last_login_time: current_time - 1000, // 1000秒前 (期限内)
        };
        assert!(!valid_session.is_expired());

        let just_expired_session = Session {
            username: "mock_user".to_string(),
            csrf_token: "mock_csrf_token".to_string(),
            session_cookie: "mock_session_cookie".to_string(),
            last_login_time: current_time - SESSION_EXPIRY,
        };
        assert!(!just_expired_session.is_expired());

        let expired_session = Session {
            username: "mock_user".to_string(),
            csrf_token: "mock_csrf_token".to_string(),
            session_cookie: "mock_session_cookie".to_string(),
            last_login_time: current_time - (SESSION_EXPIRY + 1),
        };
        assert!(expired_session.is_expired());
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

    #[tokio::test]
    async fn test_validate_login_success() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/login")
            .with_status(302)
            .with_header("content-type", "text/html")
            .with_header("Location", "/home")
            .create();
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
            .build()
            .unwrap();
        let response = client
            .post(&format!("{}/login", server.url()))
            .send()
            .await
            .unwrap();
        let result = validate_login(&response);
        assert!(result.is_ok());
        _mock.assert();
    }

    #[tokio::test]
    async fn test_validate_login_failed_wrong_status() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/login")
            .with_status(200)
            .with_header("Location", "/home")
            .create();
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
            .build()
            .unwrap();
        let response = client
            .post(&format!("{}/login", server.url()))
            .send()
            .await
            .unwrap();
        let result = validate_login(&response);
        assert!(result.is_err());
        _mock.assert();
    }

    #[tokio::test]
    async fn test_validate_login_failed_wrong_location() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/login")
            .with_status(302)
            .with_header("Location", "/login")
            .create();
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
            .build()
            .unwrap();
        let response = client
            .post(&format!("{}/login", server.url()))
            .send()
            .await
            .unwrap();
        let result = validate_login(&response);
        assert!(result.is_err());
        _mock.assert();
    }

    #[tokio::test]
    async fn test_validate_login_failed_no_location_header() {
        let mut server = Server::new_async().await;
        let _mock = server.mock("POST", "/login").with_status(302).create();
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
            .build()
            .unwrap();
        let response = client
            .post(&format!("{}/login", server.url()))
            .send()
            .await
            .unwrap();
        let result = validate_login(&response);
        assert!(result.is_err());
        _mock.assert();
    }

    #[tokio::test]
    async fn test_extract_cookie_success() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/login")
            .with_status(302)
            .with_header(
                "Set-Cookie",
                "REVEL_SESSION=mock_session_cookie; Path=/; HttpOnly",
            )
            .create();
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
            .build()
            .unwrap();
        let response = client
            .post(&format!("{}/login", server.url()))
            .send()
            .await
            .unwrap();
        let result = extract_cookie(&response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "REVEL_SESSION=mock_session_cookie");
        _mock.assert();
    }

    #[tokio::test]
    async fn test_extract_cookie_multiple_cookies() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/login")
            .with_status(302)
            .with_header("Set-Cookie", "OTHER_COOKIE=some_value; Path=/; HttpOnly")
            .with_header(
                "Set-Cookie",
                "REVEL_SESSION=mock_session_cookie; Path=/; HttpOnly",
            )
            .with_header(
                "Set-Cookie",
                "ANOTHER_COOKIE=another_value; Path=/; HttpOnly",
            )
            .create();
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
            .build()
            .unwrap();
        let response = client
            .post(&format!("{}/login", server.url()))
            .send()
            .await
            .unwrap();
        let result = extract_cookie(&response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "REVEL_SESSION=mock_session_cookie");
        _mock.assert();
    }

    #[tokio::test]
    async fn test_extract_cookie_not_found() {
        let mut server = Server::new_async().await;
        let _mock = server
            .mock("POST", "/login")
            .with_status(302)
            .with_header("Set-Cookie", "OTHER_COOKIE=some_value; Path=/; HttpOnly")
            .with_header(
                "Set-Cookie",
                "ANOTHER_COOKIE=another_value; Path=/; HttpOnly",
            )
            .create();
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
            .build()
            .unwrap();
        let response = client
            .post(&format!("{}/login", server.url()))
            .send()
            .await
            .unwrap();
        let result = extract_cookie(&response);
        assert!(result.is_err());
        _mock.assert();
    }
}
