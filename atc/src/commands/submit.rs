//! memo
//! 必須機能
//! ・atcoderにアクセスして作成したソースコードを提出する機能
//! ・提出結果リダイレクトURLをユーザに返却する機能
//! オプション機能
//! ・submit実施前にtestを実施する機能
//! ・AtCoderの採点状況をリアルタイムでターミナルに表示する機能
//!ソースコードの提出
//!
//!ソースコード (main.rs など) を読み込み、AtCoderに送信
//!提出に成功した場合、提出結果のURLを出力
//!ログインセッションの利用
//!
//!login.rs で保存したセッション (session.json) を利用
//!セッションが有効でない場合、自動的に再ログイン
//!提出成功/失敗のハンドリング
//!
//!200 OK 以外のレスポンスを適切に処理
//!AtCoderの仕様変更などで失敗した場合のエラーメッセージを明確に表示
//!提出結果の取得
//!
//!提出後、AtCoderのリダイレクトURLから submission_id を取得
//!https://atcoder.jp/contests/{contest_id}/submissions/{submission_id} を出力

use reqwest::{Client, StatusCode};
use std::{error::Error, fs, path::PathBuf};
use toml::Value;

use crate::commands::config::{BASE_URL, SESSION_FILE};
use crate::commands::login::execute as login_execute;
use crate::commands::login::Session;
/// オプション機能
/// 提出前に test を実行する
///
/// cargo atc test を実行し、すべてのテストが通った場合のみ提出
/// --force オプションでテスト失敗時でも提出可能に
/// 提出後のリアルタイム採点監視
///
/// --watch オプションで提出結果を定期的に取得
/// 結果 (AC, WA, TLE, RE など) をターミナルに表示
/// 過去の提出履歴との比較
///
/// submissions.json に過去の提出データを保存
/// 直近の提出とコードの diff を表示 (cargo atc submit --diff)
/// 提出コードのローカル保存
///
/// 提出したコードを submissions/ ディレクトリに保存 (timestamp 付き)
/// cargo atc submit --restore <ファイル名> で過去の提出を復元
/// 提出時のコンパイルオプション指定
///
/// --release オプションで cargo build --release を実行し、最適化されたバイナリを提出
/// 提出言語の指定
///
/// cargo atc submit --lang <language_id> で提出言語を選択（デフォルトはRust）
/// ブラウザで提出結果を開く
/// cargo atc submit --open で提出後に自動で提出結果ページを開く
pub async fn execute(work_dir: &PathBuf, problem_name: &str) -> Result<(), Box<dyn Error>> {
    login_execute().await?;
    let session_path = PathBuf::from(SESSION_FILE);
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
    let submission_url = submit_code(BASE_URL, &client, &session, &submission).await?;
    println!("提出成功！結果URL: {}", submission_url);
    Ok(())
}

pub struct SubmissionData {
    pub contest_name: String,
    pub problem_name: String,
    pub source_code: String,
}
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

fn read_source_code(source_path: &PathBuf) -> Result<String, Box<dyn Error>> {
    if !source_path.exists() {
        return Err(format!("ソースコードが見つかりません: {}", source_path.display()).into());
    }
    let source_code = fs::read_to_string(source_path)?;
    Ok(source_code)
}

async fn submit_code(
    base_url: &str,
    client: &Client,
    session: &Session,
    submission: &SubmissionData,
) -> Result<String, Box<dyn std::error::Error>> {
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
        .header("Referer", format!("{}/contests/{}/tasks/{}", base_url, submission.contest_name, submission.problem_name))
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .header("Cookie", cookie_header) // ✅ 修正点: 適切な `Cookie` を送信
        .form(&params)
        .send()
        .await?;

    println!("{:?}", session);
    println!("{:?}", params);
    println!("{:?}", response.status());
    if response.status() == StatusCode::FOUND {
        if let Some(location) = response.headers().get("Location") {
            let submission_url = format!("{}{}", base_url, location.to_str()?);
            return Ok(submission_url);
        }
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
        assert_eq!(
            read_source_code, source_content,
            "読み込まれたソースコードが想定と異なる"
        );
    }

    #[tokio::test]
    async fn test_submit_code_success() {
        let mut server = Server::new_async().await;
        let base_url = server.url();
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        let source_code = "fn main() { println!(\"Hello, AtCoder!\"); }";

        let _mock = server
            .mock(
                "POST",
                format!("/contests/{}/submit", contest_name).as_str(),
            )
            .match_body(Matcher::AllOf(vec![
                Matcher::Regex("csrf_token=mock_csrf_token".to_string()),
                Matcher::Regex(format!(
                    "data.TaskScreenName={}_{}",
                    contest_name, problem_name
                )),
                Matcher::Regex("data.LanguageId=5041".to_string()),
                Matcher::Regex(format!("sourceCode={}", escape(source_code))),
            ]))
            .with_status(302)
            .with_header("Location", "/submissions/123456")
            .create_async()
            .await;

        let client = Client::new();
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
        assert!(result.is_ok(), "提出処理が失敗しました");

        assert_eq!(
            result.unwrap(),
            format!("{}/submissions/123456", &base_url),
            "提出URLが正しく取得できていません"
        );
    }

    #[tokio::test]
    async fn test_atcoder() {
        let work_dir = tempfile::tempdir().expect("");
    }
}
