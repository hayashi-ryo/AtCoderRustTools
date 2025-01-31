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

pub fn execute(contest_name: &str) -> Result<(), Box<dyn Error>> {
    let base_url = "https://atcoder.jp/";
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
            .map(|el| el.text().collect::<String>().trim().to_string())
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

#[cfg(test)]
mod test {
    use std::result;

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
    }
}
