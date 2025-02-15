//! AtCoder に対するコード提出を行うためのモジュール。
//!
//! ## 主な機能
//! - `execute` - 提出処理のエントリーポイント
//! - `get_contest_info` - `Cargo.toml` からコンテスト名とソースコードのパスを取得
//! - `read_source_code` - ソースコードを読み込む
//! - `submit_code` - AtCoder にコードを提出
//!
//! ## ディレクトリ構造
//! ```text
//! .
//! ├── Cargo.toml    : AtCoder に対応する依存関係を記録したファイル
//! ├── Cargo.lock
//! └── contest_name  : 提出対象のコンテスト
//!     ├── a        : 問題 A のディレクトリ
//!     │   ├── main.rs   : 提出用のソースコード
//!     │   └── tests     : サンプルテストケース
//!     │       ├── sample_1.in
//!     │       ├── sample_1.out    
//!     │       ├── sample_2.in
//!     │       └── sample_2.out
//!     ├── b        : 問題 B のディレクトリ
//!     └── c        : 問題 C のディレクトリ
//! ```
//!
//! ## 提出フロー
//! 1. `execute` を実行すると、まず `login_execute()` により AtCoder へのログインを試行。
//! 2. `get_contest_info` により `Cargo.toml` を解析し、コンテスト名と提出対象の `main.rs` のパスを取得。
//! 3. `read_source_code` により、`main.rs` のコードを取得。
//! 4. `submit_code` を実行し、AtCoder API にコードを提出。
//! 5. 提出が成功すると、提出結果の URL を出力する。
//!
//! ## 注意事項
//! - `Cargo.toml` 内に `[bin]` セクションがない場合、エラーを返す。
//! - `submit_code` の実行時、AtCoder の CSRF トークンおよびクッキーが必要。
//! - `submit_code` のリクエストが `302 Found` を返さない場合、提出は失敗と見なされる。
//! - の提出言語 ID (`LanguageId`) は Rustの `5054` に固定されている。

use reqwest::{Client, StatusCode};
use std::{error::Error, fs, path::PathBuf};
use toml::Value;

use super::config::{get_session_file, BASE_URL};
use super::login::execute as login_execute;
use super::login::Session;

pub async fn execute(work_dir: &PathBuf, problem_name: &str) -> Result<(), Box<dyn Error>> {
    login_execute().await?;
    let session_path = get_session_file();
    let session = Session::load(&session_path)?.ok_or("セッション情報を取得できませんでした")?;

    let client = Client::new();
    // Cargo.toml から contest_name と提出対象のソースコードパスを取得
    let (contest_name, source_path) = get_contest_info(work_dir, problem_name)?;

    // ソースコードの読み込み
    let source_code = read_source_code(&PathBuf::from(&source_path))?;

    // `SubmissionData` を作成
    let submission = SubmissionData {
        contest_name,
        problem_name: problem_name.to_string(),
        source_code,
    };
    let _submission_url = submit_code(BASE_URL, &client, &session, &submission).await?;
    //println!("提出成功！結果URL: {}", submission_url);
    Ok(())
}

pub struct SubmissionData {
    pub contest_name: String,
    pub problem_name: String,
    pub source_code: String,
}

/// `Cargo.toml` からコンテスト名と提出対象のソースコードのパスを取得する
///
/// # 引数
/// - `work_dir`: `Cargo.toml` が存在する作業ディレクトリのパス。
/// - `problem_name`: 提出対象の問題名 (`a`, `b`, `c` など)。
///
/// # 戻り値
/// - `Ok((String, String))`: コンテスト名 (`contest_name`) と、提出対象のソースコードのパス (`source_path`) のタプル。
/// - `Err(Box<dyn Error>)`: `Cargo.toml` の読み込みや解析に失敗した場合、または `problem_name` に対応するエントリが存在しない場合にエラーを返す。
///
/// # 処理の流れ
/// 1. `Cargo.toml` を読み込み、TOML データとして解析。
/// 2. `[package.name]` を取得し、コンテスト名として使用。
/// 3. `[bin]` セクションを解析し、`problem_name` に対応する `path` を取得。
/// 4. 問題名が `Cargo.toml` に記載されていない場合はエラーを返す。
///
/// # エラーの可能性
/// - `Cargo.toml` が存在しない、または読み込みに失敗した場合。
/// - `[package.name]` が `Cargo.toml` に定義されていない場合。
/// - `[bin]` セクションが `Cargo.toml` に存在しない場合。
/// - 指定された `problem_name` に対応する `[[bin]]` エントリが見つからない場合。
fn get_contest_info(
    work_dir: &PathBuf,
    problem_name: &str,
) -> Result<(String, String), Box<dyn Error>> {
    let cargo_toml_path = work_dir.join("Cargo.toml");
    let cargo_toml_content = fs::read_to_string(&cargo_toml_path)?;
    let value: Value = toml::from_str(&cargo_toml_content)?;

    let contest_name = value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or("Cargo.toml にコンテスト名 (package.name) が見つかりません")?
        .to_string();
    let bins = value
        .get("bin")
        .and_then(|b| b.as_array())
        .ok_or("Cargo.toml に [bin] セクションがありません")?;

    let problem_path = bins
        .iter()
        .find_map(|bin| {
            let name = bin.get("name")?.as_str()?;
            let path = bin.get("path")?.as_str()?;
            if name == problem_name {
                Some(work_dir.join(path).to_string_lossy().to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            format!(
                "Cargo.toml に `{}` に対応するエントリが見つかりません",
                problem_name
            )
        })?;

    Ok((contest_name, problem_path))
}

/// 指定されたソースコードファイルを読み込む
///
/// # 引数
/// - `source_path`: ソースコードのファイルパス (`PathBuf`)。
///
/// # 戻り値
/// - `Ok(String)`: ソースコードの内容を `String` で返す。
/// - `Err(Box<dyn Error>)`: 読み込みに失敗した場合のエラー。
///
/// # 処理の流れ
/// 1. `source_path` が存在するかを確認し、存在しない場合はエラーを返す。
/// 2. `fs::read_to_string` を用いてソースコードを文字列として読み込む。
/// 3. 読み込んだソースコードを `Ok` で返す。
///
/// # エラーの可能性
/// - `source_path` が存在しない場合 (`ソースコードが見つかりません` エラー)。
/// - ファイルの読み込み (`fs::read_to_string`) に失敗した場合 (権限不足など)。
fn read_source_code(source_path: &PathBuf) -> Result<String, Box<dyn Error>> {
    if !source_path.exists() {
        return Err(format!("ソースコードが見つかりません: {}", source_path.display()).into());
    }
    let source_code = fs::read_to_string(source_path)?;
    Ok(source_code)
}

/// AtCoder にソースコードを提出する
///
/// # 引数
/// - `base_url`: AtCoder のベース URL (`https://atcoder.jp`)。
/// - `client`: `reqwest::Client` インスタンス (HTTP リクエストを送信するため)。
/// - `session`: `Session` 構造体 (CSRF トークンとセッション情報を保持)。
/// - `submission`: `SubmissionData` 構造体 (コンテスト名、問題名、ソースコードを含む)。
///
/// # 戻り値
/// - `Ok(String)`: 提出結果ページの URL。
/// - `Err(Box<dyn std::error::Error>)`: 提出に失敗した場合のエラー。
///
/// # 処理の流れ
/// 1. `base_url` と `contest_name` をもとに、提出 API のエンドポイント (`submit_url`) を生成。
/// 2. `submission` から問題名 (`problem_name`) とソースコード (`source_code`) を取得。
/// 3. `Session` 構造体の `csrf_token` を取得し、フォームデータ (`params`) に設定。
/// 4. `REVEL_SESSION` クッキーを `Cookie` ヘッダーに設定し、AtCoder の認証を行う。
/// 5. `client.post` を使用して AtCoder の提出 API に HTTP リクエストを送信。
/// 6. 提出成功時 (302 Found) に `Location` ヘッダーから提出結果ページの URL を取得して返す。
/// 7. 提出が失敗した場合はエラーを返す。
///
/// # エラーの可能性
/// - `Session` 情報 (`csrf_token`, `session_cookie`) が無効な場合。
/// - AtCoder の `submit_url` に HTTP リクエストが送信できなかった場合。
/// - 提出後のレスポンスの `Location` ヘッダーが存在しない場合 (予期しないレスポンス)。
async fn submit_code(
    base_url: &str,
    client: &Client,
    session: &Session,
    submission: &SubmissionData,
) -> Result<(), Box<dyn std::error::Error>> {
    let submit_url = format!("{}/contests/{}/submit", base_url, submission.contest_name);
    let params = [
        ("csrf_token", &session.csrf_token),
        (
            "data.TaskScreenName",
            &format!("{}_{}", submission.contest_name, submission.problem_name),
        ),
        ("data.LanguageId", &"5054".to_string()), // Rustの言語ID
        ("sourceCode", &submission.source_code),
    ];

    let cookie_header = format!(
        "REVEL_SESSION={};",
        session.session_cookie.trim_start_matches("REVEL_SESSION=")
    );

    let response = client
        .post(&submit_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Referer", format!("{}/contests/{}/submit", base_url, submission.contest_name))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Cookie", cookie_header) // ✅ 修正点: 適切な `Cookie` を送信
        .form(&params)
        .send()
        .await?;
    if response.status() == StatusCode::OK {
        return Ok(());
    }

    Err("提出に失敗しました".into())
}

#[cfg(test)]
mod test {
    use super::*;
    use mockito::{Matcher, Server};
    use regex::escape;
    use tempfile;

    #[test]
    fn test_get_contest_info_success() {
        let work_dir = tempfile::tempdir().expect("");
        let contest_name = "test_contest";
        let problem_name_1 = "test_1";
        let problem_name_2 = "test_2";
        let cargo_toml_path = work_dir.path().join("Cargo.toml");
        let cargo_toml_content = format!(
            r#"
[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "{}"
path = "{}/main.rs"

[[bin]]
name = "{}"
path = "{}/main.rs"
"#,
            contest_name, problem_name_1, problem_name_1, problem_name_2, problem_name_2
        );

        fs::write(&cargo_toml_path, &cargo_toml_content).expect("Cargo.toml の書き込みに失敗");
        let result = get_contest_info(&work_dir.path().to_path_buf(), problem_name_1);
        assert!(result.is_ok());
        let (get_contest_name, get_problem_path) = result.unwrap();
        assert_eq!(get_contest_name, contest_name);
        assert_eq!(
            get_problem_path,
            work_dir
                .path()
                .join(format!("{}/main.rs", problem_name_1))
                .to_string_lossy(),
        );

        let result = get_contest_info(&work_dir.path().to_path_buf(), problem_name_2);
        assert!(result.is_ok());
        let (get_contest_name, get_problem_path) = result.unwrap();
        assert_eq!(get_contest_name, contest_name);
        assert_eq!(
            get_problem_path,
            work_dir
                .path()
                .join(format!("{}/main.rs", problem_name_2))
                .to_string_lossy(),
        );
    }

    #[test]
    fn test_get_contest_info_missing_package_name() {
        let work_dir = tempfile::tempdir().expect("");
        let cargo_toml_path = work_dir.path().join("Cargo.toml");

        let cargo_toml_content = r#"
[package]
version = "0.1.0"
edition = "2021"

[[bin]]
name = "test_problem"
path = "test_problem/main.rs"
"#;

        fs::write(&cargo_toml_path, cargo_toml_content).expect("Failed to write Cargo.toml");

        let result = get_contest_info(&work_dir.path().to_path_buf(), "test_problem");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Cargo.toml にコンテスト名 (package.name) が見つかりません"
        );
    }

    #[test]
    fn test_get_contest_info_missing_bin_section() {
        let work_dir = tempfile::tempdir().expect("");
        let cargo_toml_path = work_dir.path().join("Cargo.toml");

        let cargo_toml_content = r#"
[package]
name = "test_contest"
version = "0.1.0"
edition = "2021"
"#;

        fs::write(&cargo_toml_path, cargo_toml_content).expect("Failed to write Cargo.toml");

        let result = get_contest_info(&work_dir.path().to_path_buf(), "test_problem");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Cargo.toml に [bin] セクションがありません"
        );
    }

    #[test]
    fn test_get_contest_info_problem_not_found() {
        let work_dir = tempfile::tempdir().expect("");
        let cargo_toml_path = work_dir.path().join("Cargo.toml");

        let cargo_toml_content = r#"
[package]
name = "test_contest"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "different_problem"
path = "different_problem/main.rs"
"#;

        fs::write(&cargo_toml_path, cargo_toml_content).expect("Failed to write Cargo.toml");

        let result = get_contest_info(&work_dir.path().to_path_buf(), "test_problem");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Cargo.toml に `test_problem` に対応するエントリが見つかりません"
        );
    }

    #[test]
    fn test_get_contest_info_invalid_toml() {
        let work_dir = tempfile::tempdir().expect("");
        let cargo_toml_path = work_dir.path().join("Cargo.toml");

        let invalid_cargo_toml_content = r#"
[package]
name = "test_contest"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "test_problem"
path = "test_problem/main.rs"

[[bin
name = "invalid_syntax"
"#;

        fs::write(&cargo_toml_path, invalid_cargo_toml_content)
            .expect("Failed to write Cargo.toml");

        let result = get_contest_info(&work_dir.path().to_path_buf(), "test_problem");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_source_code_success() {
        let work_dir = tempfile::tempdir().expect("");
        let problem_name = "test";
        let source_path = work_dir.path().join(format!("{}/main.rs", problem_name));
        let source_content = r#"
fn main() {
    println!("Hello, AtCoder!");
}
"#;
        fs::create_dir_all(&work_dir.path().join(problem_name)).unwrap();
        fs::write(&source_path, source_content).expect("ソースコードの書き込みに失敗");
        let result = read_source_code(&source_path);
        assert!(result.is_ok());
        let read_source_code = result.unwrap();
        assert_eq!(read_source_code, source_content);
    }

    #[test]
    fn test_read_source_code_file_not_found() {
        let work_dir = tempfile::tempdir().expect("");
        let source_path = work_dir.path().join("non_existent.rs");

        let result = read_source_code(&source_path);
        assert!(result.is_err());

        let error_message = result.unwrap_err().to_string();
        assert!(
            error_message.contains("ソースコードが見つかりません"),
            "期待するエラーメッセージと異なる: {}",
            error_message
        );
    }

    #[tokio::test]
    async fn test_submit_code_success() {
        let mut server = Server::new_async().await;
        let base_url = server.url();
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        fn encode_form_urlencoded(input: &str) -> String {
            encode(input).replace("%20", "+")
        }
        use urlencoding::encode;
        let source_code = r#"
fn main() {
    println!("Hello, AtCoder!");
}
"#;

        let _mock = server
            .mock(
                "POST",
                format!("/contests/{}/submit", contest_name).as_str(),
            )
            .match_header("Content-Type", "application/x-www-form-urlencoded")
            .match_header("Referer", format!("{}/contests/{}/submit", base_url, contest_name).as_str())
            .match_header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .match_header("Cookie", "REVEL_SESSION=mock_session_cookie;")
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("csrf_token=mock_csrf_token".to_string()),
                Matcher::Regex(format!(
                    "data.TaskScreenName={}_{}",
                    contest_name, problem_name
                )),
                Matcher::Regex("data.LanguageId=5054".to_string()),
                Matcher::Regex(format!("sourceCode={}", escape(&encode_form_urlencoded(source_code))))
            ]))
            .with_status(200)
            .with_header("Location", format!("contests/{}/submissions/me", contest_name).as_str())
            .create_async()
            .await;
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none()) // リダイレクトを無効化
            .build()
            .unwrap();
        let session = Session {
            username: "test_user".to_string(),
            csrf_token: "mock_csrf_token".to_string(),
            session_cookie: "mock_session_cookie".to_string(),
            last_login_time: 0,
        };
        let submission = SubmissionData {
            contest_name: contest_name.to_string(),
            problem_name: problem_name.to_string(),
            source_code: source_code.to_string(),
        };
        let result = submit_code(&base_url.to_string(), &client, &session, &submission).await;
        _mock.assert();
        assert!(result.is_ok());
    }
}
