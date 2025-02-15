//! テストケースの収集、資源のコンパイル、実行結果の検証、ユーザへの結果返却を行うモジュール
//!
//! このモジュールには以下の機能が含まれる。
//! - テストケースの収集(`collect_test_cases`)
//! - テスト対象資源のコンパイル(`compile`)
//! - テスト対象バイナリファイルのパス取得(`get_execution_path`)
//! - テストケースごとの実行結果の取得(`return_results`)
//!
//! このモジュールで処理対象となるディレクトリ構造は以下となる:
//! ```text
//! .
//! ├── Cargo.toml    # AtCoderに対応する依存関係を記録したファイル
//! ├── Cargo.lock
//! └── problem_name  # 入力として与える問題名
//!     ├── main.rs   # 問題に回答するロジックを実装するファイル
//!     └── tests     # AtCoderより取得したサンプル入出力を記録したディレクトリ
//!         ├── sample_1.in
//!         ├── sample_1.out    
//!         ├── sample_2.in
//!         └── sample_2.out
//! ```
use std::{
    collections::HashMap,
    error::Error,
    fmt::{Display, Formatter},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};
use toml::Value;

/// 問題名を基にテストケースの収集、資源のコンパイル、テスト結果の検証を実行する
///
/// # 引数
///
/// * `problem_name` - 処理対象となる問題名
pub fn execute(work_dir: &PathBuf, problem_name: &str) -> Result<(), Box<dyn Error>> {
    let problem_dir = find_problem_directory(&work_dir, problem_name)?;
    compile(&problem_dir)?;
    let test_cases = collect_test_cases(&problem_dir)?;
    let timeout_settings = load_problem_timeout_settings(&work_dir)?;
    let results = return_results(&work_dir, test_cases, problem_name, &timeout_settings).unwrap();

    println!("\n=== Test Results Summary ===");
    for result in &results {
        println!(
            "{}: Status = {:?}, Time = {} ms",
            result.test_case_name, result.status, result.execution_time
        );
        if let Some(error) = &result.error_message {
            println!("  Error: {}", error);
        }
    }
    println!("=============================\n");

    if results.iter().all(|res| res.status == TestStatus::AC) {
        Ok(())
    } else {
        Err("Some tests failed.".into())
    }
}

/// テストケースごとの実行結果を保持する構造体
struct TestCaseResult {
    test_case_name: String,        // サンプルケース名(例: "sample_1.in")
    status: TestStatus,            // 実行結果
    execution_time: u128,          // 実行時間(ミリ秒)
    error_message: Option<String>, // エラーが発生した場合のメッセージ
}

impl TestCaseResult {
    pub fn display_details(&self, input: &str, expected_output: &str, actual_output: &str) {
        println!("Test Case: {}", self.test_case_name);
        println!("Input:\n{}", input);
        println!("Expected Output:\n{}", expected_output);
        println!("Actual Output:\n{}", actual_output);
        println!("Status: {}", self.status);
        println!("Execution Time: {} ms\n", self.execution_time);
    }
}

/// テストケースの実行結果ステータスを表す列挙型
#[derive(PartialEq, Debug)]
enum TestStatus {
    AC,
    WA,
    TLE,
    RE,
}

impl Display for TestStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let status = match self {
            TestStatus::AC => "AC",
            TestStatus::WA => "WA",
            TestStatus::TLE => "TLE",
            TestStatus::RE => "RE",
        };
        write!(f, "{}", status)
    }
}

/// 指定された問題名に対応するディレクトリを探索する。
///
/// # 引数
///
/// * `problem_name` - 処理対象となる問題名
/// # 戻り値
///
/// ディレクトリパスを返却する。
fn find_problem_directory(
    work_dir: &PathBuf,
    problem_name: &str,
) -> Result<PathBuf, Box<dyn Error>> {
    let dir = work_dir.join(problem_name);
    if dir.exists() && dir.is_dir() {
        Ok(dir)
    } else {
        Err(format!("Directory '{}' does not exist", problem_name).into())
    }
}

/// 指定されたディレクトリ内の資源をコンパイルする
///
/// # 引数
///
/// * `dir` - コンパイル対象のディレクトリ。
fn compile(dir: &Path) -> Result<(), Box<dyn Error>> {
    let compile_status = Command::new("cargo")
        .arg("build")
        .current_dir(dir)
        .status()?;
    if compile_status.success() {
        Ok(())
    } else {
        Err("Compilation failed".into())
    }
}

/// テストケースを収集する。
///
/// 指定されたディレクトリ内の`tests`サブディレクトリから、対応する`.in`と`.out`ファイルのペアを収集する。
///
/// # 引数
///
/// * `dir` - 問題ごとのディレクトリ
///
/// # 戻り値
///
/// 成功時は`.in`と`.out`ファイルのペアを格納したベクターを返却する。
///
/// # エラー
///
/// * `tests`ディレクトリが存在しない場合。
/// * ファイルの読み込みに失敗した場合。
fn collect_test_cases(dir: &Path) -> Result<Vec<(PathBuf, PathBuf)>, Box<dyn Error>> {
    let tests_dir = dir.join("tests");
    let mut test_cases = Vec::new();
    for entry in fs::read_dir(&tests_dir)? {
        let input_file_path = entry?.path();
        if input_file_path.extension().unwrap_or_default() == "in" {
            let output_file_path = input_file_path.with_extension("out");
            if output_file_path.exists() {
                test_cases.push((input_file_path, output_file_path));
            } else {
                eprintln!("Warning: No matching output file for {:?}", input_file_path);
            }
        }
    }
    Ok(test_cases)
}

/// 問題名に基づいて実行可能ファイルのパスを取得する関数
///
/// # 引数
///
/// * `problem_name` - 処理対象となる問題名
///
/// # 戻り値
///
/// 実行可能ファイルのパスを返す。
fn get_execution_path(work_dir: &PathBuf, problem_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let executable = work_dir.join(format!("target/debug/{}", problem_name));
    if executable.exists() {
        Ok(executable)
    } else {
        Err(format!(
            "Executable for problem '{}' not found at {:?}",
            problem_name, executable
        )
        .into())
    }
}

/// Cargo.tomlから問題ごとのタイムアウト設定を取得する。
fn load_problem_timeout_settings(
    work_dir: &PathBuf,
) -> Result<HashMap<String, u64>, Box<dyn Error>> {
    let cargo_toml_path = work_dir.join("Cargo.toml");
    if !cargo_toml_path.exists() {
        return Err("Cargo.toml not found in the current directory".into());
    }

    let cargo_toml_content = fs::read_to_string(cargo_toml_path)?;
    let parsed: Value = toml::from_str(&cargo_toml_content)?;

    let timeout_section = parsed
        .get("package")
        .and_then(|pkg| pkg.get("metadata"))
        .and_then(|meta| meta.get("timeout"))
        .ok_or("Timeout section not found in Cargo.toml")?;

    let mut timeout_map = HashMap::new();
    if let Value::Table(table) = timeout_section {
        for (key, value) in table {
            if let Some(timeout) = value.as_integer() {
                timeout_map.insert(key.clone(), timeout as u64);
            }
        }
    } else {
        return Err("Timeout section is not a table".into());
    }

    Ok(timeout_map)
}

fn return_results(
    work_dir: &PathBuf,
    test_cases: Vec<(PathBuf, PathBuf)>,
    problem_name: &str,
    timeout_settings: &HashMap<String, u64>,
) -> Result<Vec<TestCaseResult>, Box<dyn Error>> {
    let executable = get_execution_path(&work_dir, problem_name)?;
    let mut results = Vec::new();
    let timeout = timeout_settings.get(problem_name).copied().unwrap();
    if !executable.exists() {
        println!("DEBUG: {}", executable.display());
        return Err(format!("Executable does not exist: {}", executable.display()).into());
    }

    for (input_file, expected_output_file) in test_cases {
        let input = fs::read_to_string(&input_file)?;
        let expected_output = fs::read_to_string(&expected_output_file)?;
        let test_case_name = input_file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let start_time = Instant::now();
        let mut child = Command::new(&executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        println!("DEBUG: {}", executable.display());
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input.as_bytes())?;
        }

        let execution_result: Result<std::process::Output, Box<dyn Error>> = loop {
            if start_time.elapsed().as_millis() > timeout as u128 {
                // TLE
                let _ = child.kill();
                break Err("Execution timed out".into());
            }
            match child.try_wait()? {
                Some(status) => {
                    if status.success() {
                        let output = child.wait_with_output()?;
                        break Ok(output);
                    } else {
                        break Err("Execution failed".into());
                    }
                }
                None => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        };
        let execution_time = start_time.elapsed().as_millis();
        let (actual_output, status, error_message) = match execution_result {
            Ok(output) => {
                let actual_output = String::from_utf8_lossy(&output.stdout).to_string();
                if actual_output.trim() == expected_output.trim() {
                    (actual_output, TestStatus::AC, None)
                } else {
                    (actual_output, TestStatus::WA, None)
                }
            }
            Err(err) => {
                if execution_time > timeout as u128 {
                    ("".to_string(), TestStatus::TLE, None)
                } else {
                    ("".to_string(), TestStatus::RE, Some(err.to_string()))
                }
            }
        };

        results.push(TestCaseResult {
            test_case_name,
            status,
            execution_time,
            error_message,
        });

        results
            .last()
            .unwrap()
            .display_details(&input, &expected_output, &actual_output);
    }
    Ok(results)
}

#[cfg(test)]
mod test {
    use super::*;
    use std::{fs, path::Path};
    use tempfile::{self, TempDir};

    #[test]
    fn find_problem_directory_success() {
        let temp_dir = tempfile::tempdir().expect("");
        let problem_name = "test_problem";
        let problem_dir = temp_dir.path().join(problem_name);
        std::fs::create_dir(&problem_dir).expect("");
        // test
        let result = find_problem_directory(&temp_dir.path().to_path_buf(), &problem_name);
        assert!(result.is_ok());
        let expected_path = problem_dir.canonicalize().expect("");
        let found_path = result.unwrap().canonicalize().expect("");
        assert_eq!(found_path, expected_path);
    }

    #[test]
    fn find_problem_directory_failed() {
        let temp_dir = tempfile::tempdir().expect("");
        let problem_name = "test_problem";
        let result = find_problem_directory(&temp_dir.path().to_path_buf(), &problem_name);

        // test
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("does not exist"));
    }

    #[test]
    fn compile_success() {
        let temp_dir = tempfile::tempdir().expect("");
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
            [package]
            name = "test_project_success"
            version = "0.1.0"
            edition = "2021"
            "#,
        )
        .unwrap();
        fs::create_dir(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("src/main.rs"),
            r#"
            fn main() {
                println!("Hello, world!");
            }
            "#,
        )
        .unwrap();

        // test
        let result = compile(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn compile_failed() {
        let temp_dir = tempfile::tempdir().expect("");
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
            [package]
            name = "test_project_failure"
            version = "0.1.0"
            edition = "2021"
            "#,
        )
        .unwrap();
        fs::create_dir(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("src/main.rs"),
            r#"
            fn main() {
                compile_error!("This is a test error.");
            }
            "#,
        )
        .unwrap();

        // test
        let result = compile(temp_dir.path());
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Compilation failed"));
    }

    #[test]
    fn collect_test_cases_success() {
        let temp_dir = tempfile::tempdir().expect("");
        std::fs::create_dir_all(temp_dir.path().join("tests")).unwrap();
        std::fs::write(temp_dir.path().join("tests/sample_1.in"), "input1").unwrap();
        std::fs::write(temp_dir.path().join("tests/sample_1.out"), "output1").unwrap();

        let test_cases = collect_test_cases(temp_dir.path()).unwrap();
        assert_eq!(test_cases.len(), 1);
        assert_eq!(test_cases[0].0.ends_with("sample_1.in"), true);
        assert_eq!(test_cases[0].1.ends_with("sample_1.out"), true);
    }

    #[test]
    fn collect_test_cases_failed() {
        let temp_dir = tempfile::tempdir().expect("");
        std::fs::create_dir_all(temp_dir.path().join("tests")).unwrap();
        std::fs::write(temp_dir.path().join("tests/sample_1.in"), "input1").unwrap();

        let test_cases = collect_test_cases(temp_dir.path()).unwrap();
        assert_eq!(test_cases.len(), 0);
    }

    #[test]
    fn load_problem_timeout_settings_success() {
        let temp_dir = tempfile::tempdir().expect("");
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        let cargo_toml_content = r#"        
        [package]
        name = "test_project"
        version = "0.1.0"
        edition = "2021"

        [package.metadata.timeout]
        a = 2000
        b = 4000
        "#;

        std::fs::create_dir_all(temp_dir.path()).unwrap();
        fs::write(cargo_toml_path, cargo_toml_content).unwrap();

        // テスト
        let timeout_settings =
            load_problem_timeout_settings(&temp_dir.path().to_path_buf()).unwrap();
        assert_eq!(timeout_settings.get("a"), Some(&2000));
        assert_eq!(timeout_settings.get("b"), Some(&4000));
    }

    #[test]
    fn load_problem_timeout_settings_failed() {
        // テスト向けファイルの準備
        let temp_dir = tempfile::tempdir().expect("");
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        let cargo_toml_content = r#"        
        [package]
        name = "test_project"
        version = "0.1.0"
        edition = "2021"

        [package.metadata.timeout]
        a = 2000
        b = 4000
        "#;
        std::fs::create_dir_all(temp_dir.path()).unwrap();
        fs::write(cargo_toml_path, cargo_toml_content).unwrap();

        // テスト

        let timeout_settings =
            load_problem_timeout_settings(&temp_dir.path().to_path_buf()).unwrap();
        assert!(timeout_settings.get("c").is_none());
    }

    /// テスト環境構築
    fn setup_test_environment(
        work_dir: &TempDir,
        test_cases: Vec<(&str, &str, &str)>,
        problem_name: &str,
        timeout: u64,
    ) -> HashMap<String, u64> {
        let problem_dir_path = work_dir.path().join(problem_name);
        let cargo_toml_path = work_dir.path().join("Cargo.toml");
        let main_rs_path = &problem_dir_path.join("main.rs");
        let cargo_toml_content = format!(
            r#"
[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "{}"
path = "{}/main.rs"

[dependencies]
proconio = "0.4.5"
    "#,
            problem_name, problem_name, problem_name
        );

        // プロジェクト構成を作成
        fs::create_dir_all(&problem_dir_path).unwrap();
        fs::write(cargo_toml_path, cargo_toml_content).unwrap();
        fs::write(
            main_rs_path,
            r#"
use proconio::input;
fn main() {
    input! {
        a: i32,
        b: i32,
    }
    println!("{}", a / b);
}
        "#,
        )
        .unwrap();

        // テストケース作成
        let test_dir_path = &problem_dir_path.join("tests");
        fs::create_dir_all(&test_dir_path).unwrap();
        for (input_name, input_content, output_content) in test_cases {
            fs::write(&test_dir_path.join(input_name), input_content).unwrap();
            let output_name = input_name.replace(".in", ".out");
            fs::write(&test_dir_path.join(output_name), output_content).unwrap();
        }

        // タイムアウト設定を返却
        let mut timeout_settings = HashMap::new();
        timeout_settings.insert(problem_name.to_string(), timeout);
        timeout_settings
    }

    /// テスト環境削除
    fn cleanup_test_environment(problem_name: &str) {
        let problem_dir = Path::new(problem_name);
        if problem_dir.exists() {
            fs::remove_dir_all(problem_dir).unwrap();
        }
    }

    #[test]
    fn return_results_ac() {
        let work_dir = tempfile::tempdir().expect("");

        // テスト環境をセットアップ
        let problem_name = "test_ac";
        let timeout_settings = setup_test_environment(
            &work_dir,
            vec![("sample_1.in", "4 2\n", "2\n")],
            problem_name,
            2000,
        );

        // テストケース収集
        let problem_dir = &work_dir.path().join(problem_name);
        let test_cases = collect_test_cases(problem_dir).unwrap();

        // プロジェクトをコンパイル
        let _ = compile(&work_dir.path());

        // テスト結果を確認
        let results = return_results(
            &work_dir.path().to_path_buf(),
            test_cases,
            problem_name,
            &timeout_settings,
        );
        assert!(results.is_ok());
        let results = results.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::AC);

        // 環境をクリーンアップ
        cleanup_test_environment(problem_name);
    }

    #[test]
    fn return_results_wa() {
        let work_dir = tempfile::tempdir().expect("");

        // テスト環境をセットアップ
        let problem_name = "test_wa";
        let timeout_settings = setup_test_environment(
            &work_dir,
            vec![("sample_1.in", "4 2\n", "0\n")],
            problem_name,
            2000,
        );

        // テストケース収集
        let problem_dir = &work_dir.path().join(problem_name);
        let test_cases = collect_test_cases(problem_dir).unwrap();

        // プロジェクトをコンパイル
        let _ = compile(&work_dir.path());

        // テスト結果を確認
        let results = return_results(
            &work_dir.path().to_path_buf(),
            test_cases,
            problem_name,
            &timeout_settings,
        );
        assert!(results.is_ok());
        let results = results.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::WA);

        // 環境をクリーンアップ
        cleanup_test_environment(problem_name);
    }

    #[test]
    fn return_results_tle() {
        let work_dir = tempfile::tempdir().expect("");

        // テスト環境をセットアップ
        let problem_name = "test_tle";
        let timeout_settings = setup_test_environment(
            &work_dir,
            vec![("sample_1.in", "4 2\n", "2\n")],
            problem_name,
            5,
        );

        // テストケース収集
        let problem_dir = &work_dir.path().join(problem_name);
        let test_cases = collect_test_cases(problem_dir).unwrap();

        // プロジェクトをコンパイル
        let _ = compile(&work_dir.path());

        // テスト結果を確認
        let results = return_results(
            &work_dir.path().to_path_buf(),
            test_cases,
            problem_name,
            &timeout_settings,
        );
        assert!(results.is_ok());
        let results = results.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::TLE);

        // 環境をクリーンアップ
        cleanup_test_environment(problem_name);
    }

    #[test]
    fn return_results_re() {
        let work_dir = tempfile::tempdir().expect("");

        // テスト環境をセットアップ
        let problem_name = "test_re";
        let timeout_settings = setup_test_environment(
            &work_dir,
            vec![("sample_1.in", "4 0\n", "2\n")],
            problem_name,
            2000,
        );

        // テストケース収集
        let problem_dir = &work_dir.path().join(problem_name);
        let test_cases = collect_test_cases(problem_dir).unwrap();

        // プロジェクトをコンパイル
        let _ = compile(&work_dir.path());

        // テスト結果を確認
        let results = return_results(
            &work_dir.path().to_path_buf(),
            test_cases,
            problem_name,
            &timeout_settings,
        );
        assert!(results.is_ok());
        let results = results.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::RE);

        // 環境をクリーンアップ
        cleanup_test_environment(problem_name);
    }
}
