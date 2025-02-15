//! AtCoder のコンテスト情報をダウンロードし、問題ごとのディレクトリ構造を作成するモジュール
//!
//! このモジュールには以下の機能が含まれる。
//! - AtCoder の問題一覧を取得 (`get_problem_list`)
//! - 各問題のディレクトリを作成 (`create_contest_directory`)
//! - `Cargo.toml` の生成 (`generate_cargo_toml`)
//! - `main.rs` のテンプレートコピー (`create_main_rs`)
//! - サンプル入出力ファイルの作成 (`create_sample_files`)
//!
//! ## ディレクトリ構造
//! このモジュールが処理対象とするディレクトリ構造は以下の通り。
//!
//! ```text
//! .
//! ├── templates               # テンプレートフォルダ (プログラム生成に利用)
//! │   ├── main.rs             # main.rs のテンプレート
//! │   └── Cargo.toml          # Cargo.toml の依存関係テンプレート
//! └── contest_name            # コンテスト名 (例: abc388)
//!     ├── Cargo.toml
//!     ├── Cargo.lock
//!     ├── a                   # 問題ごとのディレクトリ
//!     │   ├── main.rs         # 問題に回答するロジックを実装するファイル
//!     │   └── tests           # AtCoder より取得したサンプル入出力を記録したディレクトリ
//!     │       ├── sample_1.in
//!     │       ├── sample_1.out
//!     │       ├── sample_2.in
//!     │       └── sample_2.out
//!     ├── b                   # 問題 B
//!     ├── c                   # 問題 C
//!     └── ...                 # その他の問題
//! ```
//!
//! ## 主な処理
//! 1. **`fetch_html`**: AtCoder の問題ページから HTML を取得
//! 2. **`get_problem_list`**: HTML を解析し、問題一覧を取得
//! 3. **`create_contest_directory`**: コンテストのディレクトリ構造を作成
//! 4. **`generate_cargo_toml`**: `Cargo.toml` を生成し、問題ごとのバイナリ定義を追加
//! 5. **`create_main_rs`**: `templates/main.rs` をコピーし、各問題の `main.rs` を作成
//! 6. **`create_sample_files`**: AtCoder から取得したサンプル入出力ファイル (`tests/`) を作成
//!
//! ## エラーハンドリング
//! - **ネットワークエラー**: `fetch_html` で HTTP ステータスコードが `200-299` 以外の場合はエラーを返す
//! - **HTML パースエラー**: AtCoder の仕様変更により `get_problem_list` のセレクタが一致しない場合、問題一覧を取得できない
//! - **ディレクトリ作成エラー**: 無効な問題名 (`/`, `?`, `\` を含む) が渡された場合、`create_contest_directory` でエラーを返す
//! - **ファイル操作エラー**: `Cargo.toml` の作成、`main.rs` のコピー、サンプル入出力ファイルの作成時にエラーが発生する可能性がある
//!
//! このモジュールを利用することで、AtCoder のコンテスト環境を迅速にセットアップし、スムーズなコーディング環境を提供する。

use scraper::{ElementRef, Html, Selector};
use std::{
    error::Error,
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
};

use super::config::BASE_URL;

/// ダウンロード処理のエントリーポイント
pub async fn execute(work_dir: &PathBuf, contest_name: &str) -> Result<(), Box<dyn Error>> {
    let contest_info = get_problem_list(BASE_URL, &contest_name).await?;
    create_contest_directory(&work_dir, &contest_info)?;
    generate_cargo_toml(&work_dir, contest_name, &contest_info.problems)?;

    for problem in &contest_info.problems {
        create_main_rs(&work_dir, contest_name, &problem.problem_name)?;
    }

    for problem in &contest_info.problems {
        create_sample_files(
            &work_dir,
            contest_name,
            &problem.problem_name,
            &problem.samples,
        )?;
    }

    println!("Contest setup completed successfully: {}", contest_name);
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ContestInfo {
    pub contest_name: String,
    pub problems: Vec<ProblemInfo>,
}

#[derive(Debug, Clone)]
pub struct ProblemInfo {
    pub problem_name: String,
    pub timeout: u128,
    pub samples: Vec<Sample>,
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub input: String,
    pub output: String,
}

/// 指定されたURLからHTMLを取得する
///
/// # 引数
/// - `url`: 取得するページのURL
///
/// # 戻り値
/// - `Ok(String)`: HTMLの内容
/// - `Err(Box<dyn std::error::Error>)`: HTTPリクエストが失敗した場合、またはステータスコードが成功範囲(200-299)でない場合
///
/// # エラーハンドリング
/// - HTTPリクエストが失敗した場合、エラーを返す
/// - ステータスコードが 200-299 以外の場合はエラーを返す
pub async fn fetch_html(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = reqwest::get(url).await?;

    // ステータスコードが 200-299 の範囲であることを確認
    if !response.status().is_success() {
        return Err(format!("HTTP request failed with status: {}", response.status()).into());
    }

    let body = response.text().await?;
    Ok(body)
}

/// コンテストの問題一覧を取得する関数
///
/// # 引数
/// - `contest_name`: AtCoderのコンテスト名 (例: `"abc388"`)
///
/// # 戻り値
/// - `Ok(ContestInfo)`: コンテスト情報
/// - `Err(Box<dyn Error>)`: エラー時
pub async fn get_problem_list(
    base_url: &str,
    contest_name: &str,
) -> Result<ContestInfo, Box<dyn Error>> {
    let url = format!("{}/contests/{}/tasks", base_url, contest_name);
    let html = fetch_html(&url).await?;
    let document = Html::parse_document(&html);

    let row_selector = Selector::parse("tbody tr").unwrap();
    let problem_name_selector = Selector::parse("td.text-center.no-break a").unwrap();
    let timeout_selector = Selector::parse("td.text-right").unwrap();
    let link_selector = Selector::parse("td a").unwrap();

    let mut problems = Vec::new();

    for row in document.select(&row_selector) {
        let problem_name = row
            .select(&problem_name_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());
        let timeout_text = row
            .select(&timeout_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "0 sec".to_string());
        let timeout: u128 = timeout_text
            .split_whitespace()
            .next()
            .unwrap_or("0")
            .parse::<f64>()
            .map(|sec| (sec * 1000.0) as u128)
            .unwrap_or(0);
        let problem_url = row
            .select(&link_selector)
            .next()
            .map(|el| {
                format!(
                    "{}{}",
                    base_url.trim_end_matches('/'),
                    el.value().attr("href").unwrap_or("")
                )
            })
            .unwrap_or_else(|| "".to_string());
        if problem_url.is_empty() {
            continue;
        }
        let problme_html = fetch_html(&problem_url).await.unwrap();
        let problem_document = Html::parse_document(&problme_html);
        let samples = parse_samples(&problem_document).unwrap();
        problems.push(ProblemInfo {
            problem_name,
            timeout,
            samples,
        });
    }
    Ok(ContestInfo {
        contest_name: contest_name.to_string(),
        problems,
    })
}

/// AtCoderの問題ページのHTMLからサンプル入出力データを抽出する
///
/// # 引数
/// - `document`: AtCoderの問題ページの `Html` オブジェクト
///
/// # 戻り値
/// - `Ok(Vec<Sample>)`: 抽出されたサンプル入出力のリスト
/// - `Err(Box<dyn Error>)`: エラーが発生した場合
///
/// # 処理の流れ
/// 1. `h3` タグを解析し、"Sample Input" または "Sample Output" を検索する
/// 2. `pre` タグの内容を対応する入力または出力として格納する
/// 3. 入力と出力がペアになっているかを検証し、ペアが崩れている場合はエラーを返す
///
/// # エラーの可能性
/// - `h3` タグが見つからない場合 → `"入力データが見つかりません (h3タグが存在しません)"`
/// - `pre` タグが見つからない場合 → `"入力データが見つかりません (preタグが存在しません)"`
/// - 入力と出力の数が一致しない場合 → `"入出力のペアが揃っていません"`
fn parse_samples(document: &Html) -> Result<Vec<Sample>, Box<dyn Error>> {
    let h3_selector = Selector::parse("h3").unwrap();
    let mut samples = Vec::new();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut current_mode;
    let mut found_h3 = false;
    let mut found_pre = false;

    for element in document.select(&h3_selector) {
        let text = element.text().collect::<String>().trim().to_string();
        found_h3 = true;

        if text.contains("Sample Input") {
            current_mode = Some("input");
        } else if text.contains("Sample Output") {
            current_mode = Some("output");
        } else {
            current_mode = None;
        }

        let mut sibling = element.next_sibling();

        while let Some(node) = sibling {
            if let Some(el) = ElementRef::wrap(node) {
                if el.value().name() == "pre" {
                    let content = el.text().collect::<Vec<_>>().join("\n");
                    found_pre = true;

                    match current_mode {
                        Some("input") => inputs.push(content),
                        Some("output") => outputs.push(content),
                        _ => {}
                    }
                    break; // `pre` を見つけたら次の `h3` に進む
                }
            }
            sibling = node.next_sibling();
        }
    }

    if !found_h3 {
        return Err("入力データが見つかりません (h3タグが存在しません)".into());
    }
    if !found_pre {
        return Err("入力データが見つかりません (preタグが存在しません)".into());
    }
    // 入力と出力の数が合わない場合はエラー
    if inputs.len() != outputs.len() {
        return Err("入出力のペアが揃っていません".into());
    }
    for (input, output) in inputs.iter().zip(outputs.iter()) {
        samples.push(Sample {
            input: input.clone(),
            output: output.clone(),
        });
    }
    Ok(samples)
}

/// コンテストのディレクトリ構造を作成する
///
/// # 引数
/// - `work_dir`: 作業ディレクトリの `PathBuf`
/// - `contest_info`: コンテスト情報 (`ContestInfo` 構造体)
///
/// # 戻り値
/// - `Ok(())`: ディレクトリ作成が成功した場合
/// - `Err(Box<dyn Error>)`: 無効なディレクトリ名や作成失敗時のエラー
///
/// # 処理の流れ
/// 1. `contest_info.contest_name` をディレクトリ名として、コンテストディレクトリを作成
/// 2. 各問題 (`problem_name`) ごとにディレクトリを作成
/// 3. 各問題の `tests/` ディレクトリを作成
///
/// # エラーの可能性
/// - `contest_name` や `problem_name` に無効な文字（`?`, `/`, `\` など）が含まれている場合
/// - ディレクトリの作成に失敗した場合（権限不足など）
fn create_contest_directory(
    work_dir: &PathBuf,
    contest_info: &ContestInfo,
) -> Result<(), Box<dyn Error>> {
    fn is_valid_directory_name(name: &str) -> bool {
        !name.is_empty() && !name.contains('?') && !name.contains('/') && !name.contains('\\')
    }
    if !is_valid_directory_name(&contest_info.contest_name) {
        return Err("無効なディレクトリ名が指定されました".into());
    }
    let contest_dir = work_dir.join(&contest_info.contest_name);
    fs::create_dir_all(&contest_dir)?;
    for problem_info in &contest_info.problems {
        if !is_valid_directory_name(&problem_info.problem_name) {
            return Err("無効なディレクトリ名が指定されました".into());
        }
        let tests_dir = contest_dir.join(format!("{}/tests", &problem_info.problem_name));
        fs::create_dir_all(tests_dir)?;
    }

    Ok(())
}

/// コンテスト用の`Cargo.toml` を生成する
///
/// # 引数
/// - `work_dir`: 作業ディレクトリの `PathBuf`
/// - `contest_name`: コンテスト名 (`abc388` など)
/// - `problems`: コンテスト内の問題リスト (`Vec<ProblemInfo>`)
///
/// # 戻り値
/// - `Ok(())`: `Cargo.toml` の生成が成功した場合
/// - `Err(Box<dyn Error>)`: ファイル作成や書き込みに失敗した場合
///
/// # 処理の流れ
/// 1. `Cargo.toml` のパスを決定
/// 2. `template/Cargo.toml` の [dependencies] セクションを読み込み（存在する場合）
/// 3. `Cargo.toml` の [package] セクションを作成
/// 4. 各問題ごとの `[[bin]]` セクションを追加
/// 5. 各問題のタイムアウト設定 `[package.metadata.timeout]` を追加
/// 6. `Cargo.toml` を作成し、書き込み
///
/// # エラーの可能性
/// - `Cargo.toml` の作成に失敗した場合（権限不足など）
/// - `template/Cargo.toml` の読み取りに失敗した場合（ファイルが破損しているなど）
fn generate_cargo_toml(
    work_dir: &PathBuf,
    contest_name: &str,
    problems: &[ProblemInfo],
) -> Result<(), Box<dyn Error>> {
    let cargo_toml_path = work_dir.join(format!("{}/Cargo.toml", contest_name));
    let template_path = work_dir.join("template/Cargo.toml");
    let mut cargo_toml_content = String::new();
    // templateの[dependencies]を読み込む
    let mut dependencies_content = String::new();
    if template_path.exists() {
        let mut template_file = File::open(template_path)?;
        template_file.read_to_string(&mut dependencies_content)?;
    }

    // [package]
    let package_content = format!(
        r#"
[package]
name = "{}"
version = "0.1.0"
edition = "2021"
    "#,
        contest_name
    );

    // [[bin]] & [package.metadata.timeout]
    let mut bin_content = String::new();
    let mut timeout_content = String::from("\n[package.metadata.timeout]\n");
    for problem in problems {
        let problem_name = &problem.problem_name;
        bin_content.push_str(&format!(
            r#"
[[bin]]
name = "{}"
path = "{}/main.rs"
          "#,
            problem_name, problem_name
        ));

        timeout_content.push_str(&format!(
            r#""{}" = {}
"#,
            problem_name, problem.timeout
        ));
    }

    cargo_toml_content.push_str(&package_content);
    cargo_toml_content.push_str(&bin_content);
    cargo_toml_content.push_str(&timeout_content);
    cargo_toml_content.push_str("\n");
    cargo_toml_content.push_str(&dependencies_content);

    let mut file = File::create(&cargo_toml_path)?;
    file.write_all(cargo_toml_content.as_bytes())?;
    Ok(())
}

/// `main.rs` を問題ごとのディレクトリにコピーする
///
/// # 引数
/// - `work_dir`: 作業ディレクトリの `PathBuf`
/// - `contest_name`: コンテスト名 (`abc388` など)
/// - `problem_name`: 問題名 (`a`, `b`, `c` など)
///
/// # 戻り値
/// - `Ok(())`: コピー成功
/// - `Err(Box<dyn Error>)`: エラー発生時
///
/// # 処理の流れ
/// 1. `templates/main.rs` を読み込む
/// 2. コンテストディレクトリ内に `problem_name` のディレクトリを作成
/// 3. `main.rs` をコピー
///
/// # エラーの可能性
/// - `templates/main.rs` が存在しない場合
/// - ディレクトリの作成に失敗した場合
/// - ファイルのコピーに失敗した場合
fn create_main_rs(
    work_dir: &PathBuf,
    contest_name: &str,
    problem_name: &str,
) -> Result<(), Box<dyn Error>> {
    let template_path = work_dir.join("templates/main.rs");
    let problem_dir = work_dir.join(contest_name).join(problem_name);
    let main_rs_path = problem_dir.join("main.rs");

    if !template_path.exists() {
        return Err("テンプレート main.rs が見つかりません".into());
    }
    if !problem_dir.exists() {
        println!("Creating problem directory: {:?}", problem_dir);
        fs::create_dir_all(&problem_dir)?;
    }
    match fs::copy(&template_path, &main_rs_path) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Error: Failed to copy main.rs - {}", e);
            Err(e.into())
        }
    }
}

/// サンプル入出力ファイル (`tests/`) を作成する
///
/// # 引数
/// - `work_dir`: 作業ディレクトリの `PathBuf`
/// - `contest_name`: コンテスト名 (`abc388` など)
/// - `problem_name`: 問題名 (`a`, `b`, `c` など)
/// - `samples`: 入出力サンプルデータのリスト (`Vec<Sample>`)
///
/// # 戻り値
/// - `Ok(())`: 作成成功
/// - `Err(Box<dyn Error>)`: エラー発生時
///
/// # 処理の流れ
/// 1. `tests/` ディレクトリを作成
/// 2. 各サンプルの入力 (`sample_x.in`) と出力 (`sample_x.out`) ファイルを作成
/// 3. 各ファイルにサンプルデータを書き込む
///
/// # エラーの可能性
/// - `tests/` ディレクトリの作成に失敗した場合
/// - ファイルの作成や書き込みに失敗した場合
fn create_sample_files(
    work_dir: &PathBuf,
    contest_name: &str,
    problem_name: &str,
    samples: &[Sample],
) -> Result<(), Box<dyn Error>> {
    let tests_dir = work_dir.join(contest_name).join(problem_name).join("tests");
    if !tests_dir.exists() {
        println!("Creating tests directory: {:?}", tests_dir);
        fs::create_dir_all(&tests_dir)?;
    }

    for (i, sample) in samples.iter().enumerate() {
        let input_file_path = tests_dir.join(format!("sample_{}.in", i + 1));
        let output_file_path = tests_dir.join(format!("sample_{}.out", i + 1));

        // サンプル入力ファイルを作成
        println!("Creating input sample file: {:?}", input_file_path);
        let mut input_file = File::create(&input_file_path)?;
        input_file.write_all(sample.input.as_bytes())?;

        // サンプル出力ファイルを作成
        println!("Creating output sample file: {:?}", output_file_path);
        let mut output_file = File::create(&output_file_path)?;
        output_file.write_all(sample.output.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use mockito::Server;
    use std::fs;
    use tempfile;

    #[tokio::test]
    async fn test_fetch_html_success() {
        let mut server = Server::new_async().await;
        let url = format!("{}/test", server.url());
        let _get_mock = server
            .mock("GET", "/test")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html><body>Mock Response</body></html>")
            .create();

        let result = fetch_html(&url).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "<html><body>Mock Response</body></html>");
    }

    #[tokio::test]
    async fn test_fetch_html_server_error() {
        let mut server = Server::new_async().await;
        let url = format!("{}/error", server.url());
        let _get_mock = server.mock("GET", "/error").with_status(500).create();

        let result = fetch_html(&url).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_samples_success() {
        let html = Html::parse_document(
            r#"
<h3>Sample Input 1</h3><pre>Kyoto
</pre>
<h3>Sample Output 1</h3><pre>KUPC
</pre>
<h3>Sample Input 2</h3><pre>Tohoku
</pre>
<h3>Sample Output 2</h3><pre>TUPC
</pre>
        "#,
        );

        let result = parse_samples(&html);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].input, "Kyoto\n");
        assert_eq!(result[0].output, "KUPC\n");
        assert_eq!(result[1].input, "Tohoku\n");
        assert_eq!(result[1].output, "TUPC\n");
    }

    #[test]
    fn test_parse_samples_missing_output() {
        let html = Html::parse_document(
            r#"
<h3>Sample Input 1</h3><pre>Kyoto
</pre>
<h3>Sample Input 2</h3><pre>Tohoku
</pre>
<h3>Sample Output 1</h3><pre>KUPC
</pre>
    "#,
        );
        let result = parse_samples(&html);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "入出力のペアが揃っていません"
        );
    }

    #[test]
    fn test_parse_samples_missing_input() {
        let html = Html::parse_document(
            r#"
<h3>Sample Input 1</h3><pre>Kyoto
</pre>
<h3>Sample Output 1</h3><pre>KUPC
</pre>
<h3>Sample Output 2</h3><pre>TUPC
</pre>
    "#,
        );
        let result = parse_samples(&html);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "入出力のペアが揃っていません"
        );
    }

    #[test]
    fn test_parse_samples_no_pre_tags() {
        let html = Html::parse_document(
            r#"
          <h3>Sample Input 1</h3>
          <h3>Sample Output 1</h3>
    "#,
        );
        let result = parse_samples(&html);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "入力データが見つかりません (preタグが存在しません)"
        );
    }

    #[test]
    fn test_parse_samples_no_h3_tags() {
        let html = Html::parse_document(
            r#"
<pre>Kyoto
</pre>
<pre>KUPC
</pre>
<pre>Tohoku
</pre>
<pre>TUPC
</pre>
    "#,
        );
        let result = parse_samples(&html);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "入力データが見つかりません (h3タグが存在しません)"
        );
    }

    #[tokio::test]
    async fn test_get_problem_list_success() {
        let mut server = Server::new_async().await;
        let base_url = server.url();
        let contest_name = "test";
        let mock_problem_list_html = format!(
            r#"
        <html>
            <body>
                <table>
                    <tbody>
                        <tr>
                            <td class="text-center no-break"><a href="/contests/{}/tasks/{}_a">A</a></td>
                            <td class="text-right">1 sec</td>
                        </tr>
                        <tr>
                            <td class="text-center no-break"><a href="/contests/{}/tasks/{}_b">B</a></td>
                            <td class="text-right">2 sec</td>
                        </tr>
                    </tbody>
                </table>
            </body>
        </html>
    "#,
            contest_name, contest_name, contest_name, contest_name
        );

        let mock_problem_a = r#"
<h3>Sample Input 1</h3><pre>Kyoto
</pre>
<h3>Sample Output 1</h3><pre>KUPC
</pre>
<h3>Sample Input 2</h3><pre>Tohoku
</pre>
<h3>Sample Output 2</h3><pre>TUPC
</pre>
      "#;

        let mock_problem_b = r#"
<h3>Sample Input 1</h3><pre>4 3
3 3
5 1
2 4
1 10
</pre>
<h3>Sample Output 1</h3><pre>12
15
20
</pre>
<h3>Sample Input 2</h3><pre>1 4
100 100
</pre>
<h3>Sample Output 2</h3><pre>10100
10200
10300
10400
</pre>
    "#;

        let _mock_problem_list = server
            .mock("GET", format!("/contests/{}/tasks", contest_name).as_str())
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(mock_problem_list_html)
            .create();
        let _mock_problem_a = server
            .mock(
                "GET",
                format!("/contests/{}/tasks/{}_a", contest_name, contest_name).as_str(),
            )
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(mock_problem_a)
            .create();
        let _mock_problem_b = server
            .mock(
                "GET",
                format!("/contests/{}/tasks/{}_b", contest_name, contest_name).as_str(),
            )
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(mock_problem_b)
            .create();

        let result = get_problem_list(&base_url, &contest_name).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.contest_name, contest_name);
        let problem_a = &result.problems[0];
        let problem_b = &result.problems[1];

        assert_eq!(problem_a.problem_name, "a");
        assert_eq!(problem_a.timeout, 1000);
        assert_eq!(problem_a.samples[0].input, "Kyoto\n");
        assert_eq!(problem_a.samples[0].output, "KUPC\n");
        assert_eq!(problem_a.samples[1].input, "Tohoku\n");
        assert_eq!(problem_a.samples[1].output, "TUPC\n");

        assert_eq!(problem_b.problem_name, "b");
        assert_eq!(problem_b.timeout, 2000);
        assert_eq!(problem_b.samples[0].input, "4 3\n3 3\n5 1\n2 4\n1 10\n");
        assert_eq!(problem_b.samples[0].output, "12\n15\n20\n");
        assert_eq!(problem_b.samples[1].input, "1 4\n100 100\n");
        assert_eq!(problem_b.samples[1].output, "10100\n10200\n10300\n10400\n");
    }

    #[tokio::test]
    async fn test_get_problem_list_no_problems() {
        let mut server = Server::new_async().await;
        let base_url = server.url();
        let contest_name = "test_contest";
        let mock_problem_list_html = r#"
        <html>
            <body>
                <table>
                    <tbody>
                    </tbody>
                </table>
            </body>
        </html>
    "#;
        let _mock_problem_list = server
            .mock("GET", format!("/contests/{}/tasks", contest_name).as_str())
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(mock_problem_list_html)
            .create();

        let result = get_problem_list(&base_url, contest_name).await;
        assert!(result.is_ok());
        let contest_info = result.unwrap();
        assert_eq!(contest_info.contest_name, contest_name);
        assert!(contest_info.problems.is_empty());
    }

    #[tokio::test]
    async fn test_get_problem_list_invalid_html() {
        let mut server = Server::new_async().await;
        let base_url = server.url();
        let contest_name = "test_contest";

        let mock_invalid_html = r#"
        <html>
            <body>
                <div>Invalid HTML</div>
            </body>
        </html>
    "#;

        let _mock_problem_list = server
            .mock("GET", format!("/contests/{}/tasks", contest_name).as_str())
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(mock_invalid_html)
            .create();

        let result = get_problem_list(&base_url, contest_name).await;
        assert!(result.is_ok());
        let contest_info = result.unwrap();
        assert_eq!(contest_info.contest_name, contest_name);
        assert!(contest_info.problems.is_empty());
    }

    #[tokio::test]
    async fn test_get_problem_list_network_error() {
        let mut server = Server::new_async().await;
        let base_url = server.url();
        let contest_name = "test_contest";

        let _mock_problem_list = server
            .mock("GET", format!("/contests/{}/tasks", contest_name).as_str())
            .with_status(500) // Internal Server Error
            .create();

        let result = get_problem_list(&base_url, contest_name).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_create_contest_directory_success() {
        let work_dir = tempfile::tempdir().expect("");
        let contest_name = "test_contest";
        let sample = vec![
            Sample {
                input: "Kyoto".to_string(),
                output: "KUPC".to_string(),
            },
            Sample {
                input: "Tohoku".to_string(),
                output: "TUPC".to_string(),
            },
        ];
        let contest_info = ContestInfo {
            contest_name: contest_name.to_string(),
            problems: vec![
                ProblemInfo {
                    problem_name: "test_1".to_string(),
                    timeout: 1000,
                    samples: sample.clone(),
                },
                ProblemInfo {
                    problem_name: "test_2".to_string(),
                    timeout: 2000,
                    samples: sample.clone(),
                },
            ],
        };

        let result = create_contest_directory(&work_dir.path().to_path_buf(), &contest_info);
        assert!(result.is_ok());

        // 結果の確認
        let contest_dir = work_dir.path().join(contest_name);
        assert!(contest_dir.exists() && contest_dir.is_dir());

        for problem in &contest_info.problems {
            let problem_dir = contest_dir.join(&problem.problem_name);
            let tests_dir = problem_dir.join("tests");

            assert!(problem_dir.exists() && problem_dir.is_dir());
            assert!(tests_dir.exists() && tests_dir.is_dir());
        }
    }

    #[test]
    fn test_create_contest_directory_invalid_path() {
        let work_dir = tempfile::tempdir().expect("");
        let contest_info = ContestInfo {
            contest_name: "test?1".to_string(),
            problems: vec![ProblemInfo {
                problem_name: "a".to_string(),
                timeout: 1000,
                samples: vec![],
            }],
        };
        let result = create_contest_directory(&work_dir.path().to_path_buf(), &contest_info);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_contest_directory_invalid_characters() {
        let work_dir = tempfile::tempdir().expect("");
        let contest_info = ContestInfo {
            contest_name: "test".to_string(),
            problems: vec![ProblemInfo {
                problem_name: "b/c".to_string(), // 不正な文字を含む
                timeout: 2000,
                samples: vec![],
            }],
        };
        let result = create_contest_directory(&work_dir.path().to_path_buf(), &contest_info);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_cargo_toml_success() {
        let work_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let contest_name = "test_contest";
        let cargo_toml_path = work_dir.path().join(format!("{}/Cargo.toml", contest_name));

        let problems = vec![
            ProblemInfo {
                problem_name: "a".to_string(),
                timeout: 2000,
                samples: vec![],
            },
            ProblemInfo {
                problem_name: "b".to_string(),
                timeout: 2500,
                samples: vec![],
            },
        ];
        let contest_dir = work_dir.path().join(contest_name);
        let _ = fs::create_dir_all(contest_dir);
        let result = generate_cargo_toml(&work_dir.path().to_path_buf(), contest_name, &problems);
        assert!(result.is_ok());
        assert!(cargo_toml_path.exists());
        let cargo_content = fs::read_to_string(&cargo_toml_path).unwrap();
        assert!(cargo_content.contains("[package]"));
        assert!(cargo_content.contains("name = \"test_contest\""));
        assert!(cargo_content.contains("version = \"0.1.0\""));
        assert!(cargo_content.contains("[[bin]]"));
        assert!(cargo_content.contains("name = \"a\""));
        assert!(cargo_content.contains("path = \"a/main.rs\""));
        assert!(cargo_content.contains("[package.metadata.timeout]"));
        assert!(cargo_content.contains("\"a\" = 2000"));
        assert!(cargo_content.contains("\"b\" = 2500"));
    }

    #[test]
    fn test_create_main_rs_success() {
        let work_dir = tempfile::tempdir().expect("");
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        let contest_path = work_dir.path().join(contest_name);
        let problem_path = contest_path.join(problem_name);
        let main_rs_path = problem_path.join("main.rs");
        let template_path = work_dir.path().join("templates/main.rs");

        // テンプレート `main.rs` を作成
        fs::create_dir_all(work_dir.path().join("templates")).unwrap();
        fs::write(template_path, "fn main() { println!(\"Hello, world!\"); }").unwrap();

        // 実行
        let result = create_main_rs(&work_dir.path().to_path_buf(), contest_name, problem_name);
        assert!(result.is_ok());

        // `main.rs` が作成されているか確認
        assert!(main_rs_path.exists());

        // `main.rs` の内容を確認
        let content = fs::read_to_string(&main_rs_path).unwrap();
        assert_eq!(content, "fn main() { println!(\"Hello, world!\"); }");
    }

    #[test]
    fn test_create_main_rs_missing_template() {
        let work_dir = tempfile::tempdir().expect("");
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        let result = create_main_rs(&work_dir.path().to_path_buf(), contest_name, problem_name);

        // `templates/main.rs` が存在しない場合、エラーになることを確認
        assert!(result.is_err());
    }

    #[test]
    fn test_create_sample_files_success() {
        let work_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        let tests_path = work_dir
            .path()
            .join(contest_name)
            .join(problem_name)
            .join("tests");

        let samples = vec![
            Sample {
                input: "Kyoto".to_string(),
                output: "KUPC".to_string(),
            },
            Sample {
                input: "Tohoku".to_string(),
                output: "TUPC".to_string(),
            },
        ];

        let result = create_sample_files(
            &work_dir.path().to_path_buf(),
            contest_name,
            problem_name,
            &samples,
        );
        assert!(result.is_ok());
        for (i, sample) in samples.iter().enumerate() {
            let input_file_path = tests_path.join(format!("sample_{}.in", i + 1));
            let output_file_path = tests_path.join(format!("sample_{}.out", i + 1));

            assert!(input_file_path.exists());
            assert!(output_file_path.exists());

            let input_content = fs::read_to_string(&input_file_path).unwrap();
            let output_content = fs::read_to_string(&output_file_path).unwrap();

            assert_eq!(input_content, sample.input);
            assert_eq!(output_content, sample.output);
        }
    }
}
