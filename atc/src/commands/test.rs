//! テストケースの収集、資源のコンパイル、実行結果の検証、ユーザへの結果返却を行うモジュール
//!
//! このモジュールには以下の機能が含まれる。
//! - テストケースの収集(`collect_test_cases)
//! - テスト対象資源のコンパイル(`compile`)
//! - テスト対象バイナリファイルのパス取得(`get_execution_path`)
//! - 実行結果の標準出力キャプチャ(`get_execution_output`)
//! - ユーザへの返却(`return_results`)
//!
//! このモジュールで処理対象となるディレクトリ構造は以下となる:
//! .
//! ├── Cargo.toml    : AtCoderに対応する依存関係を記録したファイル
//! ├── Cargo.lock
//! └── problem_name  : 入力として与える問題名
//!     ├── main.rs   : 問題に回答するロジックを実装するファイル
//!     └── tests     : AtCoderより取得したサンプル入出力を記録したディレクトリ
//!         ├── sample_1.in
//!         ├── sample_1.out    
//!         ├── sample_2.in
//!         └── sample_2.out
//!
//! memo
//! exexute
//! 1. find_problem_directoryで問題格納ディレクトリの取得
//! 2. compileでproblem_name/main.rsをコンパイル
//! 3. collect_test_casesでテストケースを取得してvecに格納
//! 4. timeout_settingsでcontest_nameのtimeout設定をhashmapとして取得
//! 5. return_resultsでユーザに結果を返却
//!     1. 実行ファイルの取得
//!     2. テストケースごとに実行
//!     3. resultvecに格納
//! 6. ユーザに結果を返却
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
pub fn execute(problem_name: &str) -> Result<(), Box<dyn Error>> {
    let project_root = std::env::current_dir()?;
    let dir = find_problem_directory(problem_name)?;
    compile(&dir)?;
    let test_cases = collect_test_cases(&dir)?;
    let timeout_settings = load_problem_timeout_settings()?;
    return_results(test_cases, problem_name, &timeout_settings)?;
    Ok(())
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
#[derive(PartialEq)]
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
fn find_problem_directory(problem_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let dir = Path::new("./").join(problem_name);
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
/// * `dir` - テストケースが格納されている問題ディレクトリ。
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
fn get_execution_path(problem_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let project_root = std::env::current_dir()?;
    let executable = project_root.join(format!("target/debug/{}", problem_name));
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
fn load_problem_timeout_settings() -> Result<HashMap<String, u64>, Box<dyn Error>> {
    let cargo_toml_path = Path::new("Cargo.toml");
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
    test_cases: Vec<(PathBuf, PathBuf)>,
    problem_name: &str,
    timeout_settings: &HashMap<String, u64>,
) -> Result<(), Box<dyn Error>> {
    // 実行可能ファイルのパスを取得
    let executable = get_execution_path(problem_name)?;

    let mut results = Vec::new();

    for (input_file, expected_output_file) in test_cases {
        let input = fs::read_to_string(&input_file)?;
        let expected_output = fs::read_to_string(&expected_output_file)?;
        let test_case_name = input_file
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        let timeout = timeout_settings
            .get(&test_case_name)
            .copied()
            .unwrap_or(2000); // デフォルト 2000ms

        let start_time = Instant::now();
        let execution_result = Command::new(&executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(input.as_bytes())?;
                }
                child.wait_with_output()
            });

        let execution_time = start_time.elapsed().as_millis();
        let (actual_output, status, error_message) = match execution_result {
            Ok(output) => {
                let actual_output = String::from_utf8_lossy(&output.stdout).to_string();
                if execution_time > timeout as u128 {
                    (actual_output, TestStatus::TLE, None)
                } else if actual_output.trim() == expected_output.trim() {
                    (actual_output, TestStatus::AC, None)
                } else {
                    (actual_output, TestStatus::WA, None)
                }
            }
            Err(err) => ("".to_string(), TestStatus::RE, Some(err.to_string())),
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
    println!("\n=== Test Results Summary ===");
    for result in &results {
        println!(
            "{}: Status = {}, Time = {} ms",
            result.test_case_name, result.status, result.execution_time
        );
    }
    println!("=============================\n");

    if results.iter().all(|res| res.status == TestStatus::AC) {
        Ok(())
    } else {
        Err("Some tests failed.".into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn execute_success() {
        // preparation
        let test_dir = "test_execute";
        std::fs::create_dir_all(format!("{}/tests", test_dir)).unwrap();
        std::fs::write(
            format!("{}/Cargo.toml", test_dir),
            r#"
            [package]
            name = "test_execute"
            version = "0.1.0"
            edition = "2021"
            
            [dependencies]
            proconio = "0.4.5"
        "#,
        )
        .unwrap();
        std::fs::create_dir_all(format!("{}/src", test_dir)).unwrap();
        std::fs::write(
            format!("{}/src/main.rs", test_dir),
            r#"
                use proconio::{input, marker::Chars};

                fn main() {
                    input! {
                        s1: String,
                        s2: String
                    }
                    println!("{} {}", s1, s2);
                }
        "#,
        )
        .unwrap();
        std::fs::write(
            format!("{}/tests/sample_1.in", test_dir),
            "Hello, AtCoder!\n",
        )
        .unwrap();
        std::fs::write(
            format!("{}/tests/sample_1.out", test_dir),
            "Hello, AtCoder!\n",
        )
        .unwrap();

        // test
        let result = execute("test_execute");
        assert!(result.is_ok());

        // post-process
        std::fs::remove_dir_all(test_dir).unwrap();
    }

    #[test]
    fn find_problem_directory_success() {
        // preparation
        let temp_dir = TempDir::new_in(".").expect("Error");
        let dir_name = temp_dir
            .path()
            .file_name()
            .expect("")
            .to_string_lossy()
            .to_string();

        // test
        let result = find_problem_directory(&dir_name);
        println!("PATH: {}", dir_name);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert_eq!(path.to_str().unwrap(), format!("./{}", dir_name));
    }

    #[test]
    fn find_problem_directory_failed() {
        let result = find_problem_directory("non_existent_dir");

        // test
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("does not exist"));
    }

    #[test]
    fn compile_success() {
        // preparation
        let temp_dir = TempDir::new_in(".").expect("Error");
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
        // preparation
        let temp_dir = TempDir::new_in(".").expect("Error");
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
        let temp_dir = TempDir::new_in(".").expect("Error");
        //std::fs::create_dir_all(test_dir).unwrap();
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
        let temp_dir = TempDir::new_in(".").expect("Error");
        std::fs::create_dir_all(temp_dir.path().join("tests")).unwrap();
        std::fs::write(temp_dir.path().join("tests/sample_1.in"), "input1").unwrap();

        let test_cases = collect_test_cases(temp_dir.path()).unwrap();
        assert_eq!(test_cases.len(), 0);
    }

    #[test]
    fn load_problem_timeout_settings_success() {
        // テスト向けファイルの準備
        let temp_dir = TempDir::new_in(".").expect("Error");
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
        std::env::set_current_dir(&temp_dir.path()).unwrap();
        fs::write("Cargo.toml", cargo_toml_content).unwrap();

        // テスト

        let timeout_settings = load_problem_timeout_settings().unwrap();
        assert_eq!(timeout_settings.get("a"), Some(&2000));
        assert_eq!(timeout_settings.get("b"), Some(&4000));
    }

    #[test]
    fn load_problem_timeout_settings_failed() {
        // テスト向けファイルの準備
        let temp_dir = TempDir::new_in(".").expect("Error");
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
        std::env::set_current_dir(&temp_dir).unwrap();
        fs::write("Cargo.toml", cargo_toml_content).unwrap();

        // テスト

        let timeout_settings = load_problem_timeout_settings().unwrap();
        assert!(timeout_settings.get("c").is_none());
    }
}
