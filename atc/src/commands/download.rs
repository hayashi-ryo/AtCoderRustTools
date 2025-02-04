//! memo
//! 情報取得とファイル生成の処理を分割して実装を進めていく
//! - 情報取得
//!   - 問題一覧の情報取得
//!   - 問題ごとの詳細情報取得
//!   - AtCoderページアクセス
//!   - HTMLから特定の情報を抽出
//!   - サンプルデータのパース
//! - ファイル生成
//!   - Cargo.tomlの生成
//!   - testsファイル配下のサンプル入出力ファイルの生成

use scraper::{ElementRef, Html, Selector};
use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub async fn execute(work_dir: &PathBuf, contest_name: &str) -> Result<(), Box<dyn Error>> {
    let base_url = "https://atcoder.jp";

    // 1. コンテストの問題一覧を取得
    let contest_info = get_problem_list(&base_url, &contest_name).await?;
    println!("ContestInfo: {:?}", contest_info);
    // 2. 各問題のディレクトリ構造を作成
    for problem in &contest_info.problems {
        create_contest_directory(contest_name, &problem.problem_name)?;
    }

    // 3. `Cargo.toml` を生成
    generate_cargo_toml(contest_name, &contest_info.problems)?;

    // 4. `main.rs` をコピー
    for problem in &contest_info.problems {
        create_main_rs(contest_name, &problem.problem_name)?;
    }

    // 5. サンプル入出力ファイル (`tests/`) を作成
    for problem in &contest_info.problems {
        create_sample_files(contest_name, &problem.problem_name, &problem.samples)?;
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

/// ページにアクセスしてHTMLを取得する関数
///
/// # 引数
/// - `url`: 取得するページのURL
///
/// # 戻り値
/// - `Ok(String)`: HTMLの内容
/// - `Err(reqwest::Error)`: エラーが発生した場合
pub async fn fetch_html(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = reqwest::get(url).await?;
    let body = response.text().await?;
    Ok(body)
}

/// 実行時間制限を取得する関数
///
/// # 引数
/// - `document`: AtCoderの問題ページのHTML
///
/// # 戻り値
/// - `Ok(u128)`: 取得した実行時間制限 (ms)
/// - `Err(Box<dyn Error>)`: エラー時
fn get_timeout(document: &Html) -> Result<u128, Box<dyn Error>> {
    let selector = Selector::parse("p").unwrap();
    for p in document.select(&selector) {
        let text = p.text().collect::<String>().trim().to_string();
        if text.contains("Time Limit:") {
            let timeout_text = text
                .split(":")
                .nth(1)
                .ok_or("実行時間制限のフォーマットが不正です")?
                .trim()
                .split_whitespace()
                .next()
                .ok_or("実行時間制限の値が見つかりません")?;
            let timeout_sec: f64 = timeout_text
                .parse()
                .map_err(|_| "実行時間制限の数値変換に失敗しました")?;
            let timeout_ms = (timeout_sec * 1000.0) as u128;

            return Ok(timeout_ms);
        }
    }
    Err("実行時間制限が見つかりません".into())
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

/// サンプルデータを取得する関数
///
/// # 引数
/// - `document`: AtCoderの問題ページのHTML
///
/// # 戻り値
///
/// - `Ok(Vec<Sample>)`: サンプル入出力のリスト
/// - `Err(dyn Error)`: エラーが発生した場合
fn parse_samples(document: &Html) -> Result<Vec<Sample>, Box<dyn Error>> {
    let h3_selector = Selector::parse("h3").unwrap();
    let mut samples = Vec::new();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut current_mode = None;
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

fn create_contest_directory(contest_name: &str, problem_name: &str) -> Result<(), Box<dyn Error>> {
    fn is_valid_directory_name(name: &str) -> bool {
        !name.is_empty() && !name.contains('?') && !name.contains('/') && !name.contains('\\')
    }
    if !is_valid_directory_name(contest_name) || !is_valid_directory_name(problem_name) {
        return Err("無効なディレクトリ名が指定されました".into());
    }

    let contest_dir = Path::new(contest_name);
    let tests_dir = contest_dir.join(format!("{}/tests", problem_name));
    fs::create_dir_all(contest_dir)?;
    fs::create_dir_all(tests_dir)?;

    Ok(())
}

fn generate_cargo_toml(contest_name: &str, problems: &[ProblemInfo]) -> Result<(), Box<dyn Error>> {
    let cargo_toml_path = PathBuf::from(contest_name).join("Cargo.toml");
    let template_path = PathBuf::from("templates/Cargo.toml");
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

fn create_main_rs(contest_name: &str, problem_name: &str) -> Result<(), Box<dyn Error>> {
    let template_path = Path::new("templates/main.rs");
    let problem_dir = Path::new(contest_name).join(problem_name);
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

fn create_sample_files(
    contest_name: &str,
    problem_name: &str,
    samples: &[Sample],
) -> Result<(), Box<dyn Error>> {
    let tests_dir = Path::new(contest_name).join(problem_name).join("tests");
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
    async fn test_fetch_html_not_found() {
        let mut server = Server::new_async().await;
        let url = format!("{}/not_found", server.url());
        let _get_mock = server.mock("GET", "/not_found").with_status(404).create();
        let result = fetch_html(&url).await;
        assert!(result.is_ok());
        _get_mock.assert();
    }

    #[test]
    fn test_get_timeout_success() {
        let html = r#"
        		<p>
			        Time Limit: 2 sec / Memory Limit: 1024 MB			
		        </p>
        "#;
        let html = Html::parse_document(html);
        let result = get_timeout(&html);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result, 2000);
    }

    #[test]
    fn test_get_timeout_failed() {
        let html = r#"
        		<p>
			        Sample Sample
		        </p>
        "#;
        let html = Html::parse_document(html);
        let result = get_timeout(&html);
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
							              <td><a href="/contests/{}/tasks/{}_a">The First Problem</a></td>
							              <td class="text-right">1 sec</td>
							              <td class="text-right">1024 MB</td>
                        </tr>
                        <tr>
                            <td class="text-center no-break"><a href="/contests/{}/tasks/{}_b">B</a></td>
                            <td class="text-right">4 sec</td>
							              <td class="text-right">1024 MB</td>
                        </tr>
                    </tbody>
                </table>
            </body>
        </html>
    "#,
            contest_name, contest_name, contest_name, contest_name, contest_name, contest_name
        );

        let mock_problem_a = r#"
<title>A</title>
<p>
  Time Limit: 2 sec / Memory Limit: 1024 MB			
</p>
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
<p>
Time Limit: 4 sec / Memory Limit: 1024 MB			
</p>
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

        // let base_url = "https://atcoder.jp";
        // let contest_name = "abc388";
        let result = get_problem_list(&base_url, &contest_name).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.contest_name, contest_name);
        let problem_a = &result.problems[0];
        let problem_b = &result.problems[1];

        assert_eq!(problem_a.problem_name, "A");
        assert_eq!(problem_a.timeout, 1000);
        assert_eq!(problem_a.samples[0].input, "Kyoto\n");
        assert_eq!(problem_a.samples[0].output, "KUPC\n");
        assert_eq!(problem_a.samples[1].input, "Tohoku\n");
        assert_eq!(problem_a.samples[1].output, "TUPC\n");

        assert_eq!(problem_b.problem_name, "B");
        assert_eq!(problem_b.timeout, 4000);
        assert_eq!(problem_b.samples[0].input, "4 3\n3 3\n5 1\n2 4\n1 10\n");
        assert_eq!(problem_b.samples[0].output, "12\n15\n20\n");
        assert_eq!(problem_b.samples[1].input, "1 4\n100 100\n");
        assert_eq!(problem_b.samples[1].output, "10100\n10200\n10300\n10400\n");
        println!("{:?}", result);
    }

    #[test]
    fn test_create_contest_directory_success() {
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        let contest_path = Path::new(contest_name);
        let tests_path = contest_path.join(format!("{}/tests", problem_name));

        // 実行
        let result = create_contest_directory(contest_name, problem_name);

        // 結果の確認
        assert!(result.is_ok());
        assert!(contest_path.exists());
        assert!(tests_path.exists());

        fs::remove_dir_all(contest_path).unwrap();
    }

    #[test]
    fn test_create_contest_directory_invalid_path() {
        let contest_name = "";
        let problem_name = "test_problem";
        let result = create_contest_directory(contest_name, problem_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_contest_directory_invalid_characters() {
        let contest_name = "invalid?contest";
        let problem_name = "test_problem";
        let result = create_contest_directory(contest_name, problem_name);
        assert!(result.is_err());
    }

    use std::{fs, path::Path};
    use tempfile;
    #[test]
    fn test_generate_cargo_toml_success() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        std::env::set_current_dir(&temp_dir).unwrap();
        let contest_name = "test_contest";
        let cargo_toml_path = PathBuf::from(contest_name).join("Cargo.toml");

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
        let contest_dir = Path::new(contest_name);
        let _ = fs::create_dir_all(contest_dir);
        let result = generate_cargo_toml(contest_name, &problems);
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
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        std::env::set_current_dir(&temp_dir).unwrap();
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        let contest_path = PathBuf::from(contest_name);
        let problem_path = contest_path.join(problem_name);
        let main_rs_path = problem_path.join("main.rs");
        let template_path = Path::new("templates/main.rs");

        // テンプレート `main.rs` を作成
        fs::create_dir_all("templates").unwrap();
        fs::write(template_path, "fn main() { println!(\"Hello, world!\"); }").unwrap();

        // 実行
        let result = create_main_rs(contest_name, problem_name);
        assert!(result.is_ok());

        // `main.rs` が作成されているか確認
        assert!(main_rs_path.exists());

        // `main.rs` の内容を確認
        let content = fs::read_to_string(&main_rs_path).unwrap();
        assert_eq!(content, "fn main() { println!(\"Hello, world!\"); }");

        // クリーンアップ
        fs::remove_dir_all(&contest_path).unwrap();
        fs::remove_file(template_path).unwrap();
    }

    #[test]
    fn test_create_main_rs_missing_template() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        std::env::set_current_dir(&temp_dir).unwrap();
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        let result = create_main_rs(contest_name, problem_name);

        // `templates/main.rs` が存在しない場合、エラーになることを確認
        assert!(result.is_err());
    }

    #[test]
    fn test_create_sample_files_success() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        std::env::set_current_dir(&temp_dir).unwrap();
        let contest_name = "test_contest";
        let problem_name = "test_problem";
        let tests_path = PathBuf::from(contest_name).join(problem_name).join("tests");

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

        let result = create_sample_files(contest_name, problem_name, &samples);
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
