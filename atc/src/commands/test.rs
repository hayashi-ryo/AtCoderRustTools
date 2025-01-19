use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

pub fn execute(problem_name: &str) -> Result<(), Box<dyn Error>> {
    let dir = find_problem_directory(problem_name)?;
    compile(&dir)?;
    let test_cases = collect_test_cases(&dir)?;
    let executable = dir.join("target/debug/test_project");
    return_results(test_cases, &executable)?;
    Ok(())
}

fn find_problem_directory(problem_name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let dir = Path::new(".").join(problem_name);
    if dir.exists() && dir.is_dir() {
        Ok(dir)
    } else {
        Err(format!("Directory '{}' does not exist", problem_name).into())
    }
}

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

fn collect_test_cases(dir: &Path) -> Result<Vec<(PathBuf, PathBuf)>, Box<dyn Error>> {
    let mut test_cases = Vec::new();
    for entry in fs::read_dir(dir)? {
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

#[allow(dead_code)]
fn measure_execution_time(executable: &Path, input_file: &Path) -> Result<u128, Box<dyn Error>> {
    let input_data = fs::read_to_string(input_file)?;
    let start = Instant::now();
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(input_data.as_bytes())?;
    }

    child.wait()?;
    let duration = start.elapsed().as_millis();
    Ok(duration)
}

fn get_execution_output(executable: &Path, input_file: &Path) -> Result<String, Box<dyn Error>> {
    let input_data = fs::read_to_string(input_file)?;
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input_data.as_bytes())?;
    }
    let output = child.wait_with_output()?.stdout;
    let mut output_str = String::from_utf8_lossy(&output).to_string();
    if !output_str.ends_with('\n') {
        output_str.push('\n');
    }

    Ok(output_str)
}

fn validate_output(actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "Output mismatch:\nExpected:\n{}\nActual:\n{}",
            expected, actual
        ))
    }
}

fn return_results(
    test_cases: Vec<(PathBuf, PathBuf)>,
    executable: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut all_successful = true;
    for (input_file, expected_output_file) in test_cases {
        let actual_output = get_execution_output(executable, &input_file)?;
        let expected_output = fs::read_to_string(&expected_output_file)?;

        match validate_output(&actual_output, &expected_output) {
            Ok(_) => {
                println!(
                    "Test passed: Input: {:?}, Output: {:?}",
                    input_file, expected_output_file
                );
            }
            Err(error) => {
                eprintln!(
                    "Test failed: Input: {:?}, Expected Output: {:?}\nError: {}",
                    input_file, expected_output_file, error
                );
                all_successful = false;
            }
        }
    }

    if all_successful {
        Ok(())
    } else {
        Err("Some tests failed.".into())
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use std::fs;

    #[test]
    fn execute_success() {
        let test_dir = "test_execute";
        std::fs::create_dir_all(test_dir).unwrap();
        std::fs::write(
            format!("{}/Cargo.toml", test_dir),
            r#"
            [package]
            name = "test_execute"
            version = "0.1.0"
            edition = "2021"
        "#,
        )
        .unwrap();
        std::fs::create_dir_all(format!("{}/src", test_dir)).unwrap();
        std::fs::write(
            format!("{}/src/main.rs", test_dir),
            r#"
            fn main() {
                println!("Hello, AtCoder!");
            }
        "#,
        )
        .unwrap();
        std::fs::write(format!("{}/sample_1.in", test_dir), "input1").unwrap();
        std::fs::write(format!("{}/sample_1.out", test_dir), "Hello, AtCoder!\n").unwrap();

        let result = execute("test_execute");
        assert!(result.is_ok());

        std::fs::remove_dir_all(test_dir).unwrap();
    }
    #[test]
    fn find_problem_directory_success() {
        // preparation
        let unit_test_dir = "unit_test";
        fs::create_dir_all(unit_test_dir).unwrap();

        // test
        let result = find_problem_directory(unit_test_dir);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert_eq!(path.to_str().unwrap(), format!("./{}", "unit_test"));

        // post-process
        fs::remove_dir(unit_test_dir).unwrap();
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
        let test_dir = Path::new("test_project_success");
        fs::create_dir(test_dir).unwrap();
        fs::write(
            test_dir.join("Cargo.toml"),
            r#"
            [package]
            name = "test_project_success"
            version = "0.1.0"
            edition = "2021"
            "#,
        )
        .unwrap();
        fs::create_dir(test_dir.join("src")).unwrap();
        fs::write(
            test_dir.join("src/main.rs"),
            r#"
            fn main() {
                println!("Hello, world!");
            }
            "#,
        )
        .unwrap();

        // test
        let result = compile(test_dir);
        assert!(result.is_ok());

        // post-process
        fs::remove_dir_all(test_dir).unwrap();
        let _executable_clear = Command::new("cargo")
            .arg("clean")
            .current_dir(test_dir)
            .status();
    }

    #[test]
    fn compile_failed() {
        // preparation
        let test_dir = Path::new("test_project_failure");
        fs::create_dir(test_dir).unwrap();
        fs::write(
            test_dir.join("Cargo.toml"),
            r#"
            [package]
            name = "test_project_failure"
            version = "0.1.0"
            edition = "2021"
            "#,
        )
        .unwrap();
        fs::create_dir(test_dir.join("src")).unwrap();
        fs::write(
            test_dir.join("src/main.rs"),
            r#"
            fn main() {
                compile_error!("This is a test error.");
            }
            "#,
        )
        .unwrap();

        // test
        let result = compile(test_dir);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("Compilation failed"));

        // post-process
        fs::remove_dir_all(test_dir).unwrap();
    }

    #[test]
    fn collect_test_cases_success() {
        let test_dir = "collect_test_cases_success";
        std::fs::create_dir_all(test_dir).unwrap();
        std::fs::write(format!("{}/sample_1.in", test_dir), "input1").unwrap();
        std::fs::write(format!("{}/sample_1.out", test_dir), "output1").unwrap();

        let test_cases = collect_test_cases(Path::new(test_dir)).unwrap();

        assert_eq!(test_cases.len(), 1);
        assert_eq!(test_cases[0].0.ends_with("sample_1.in"), true);
        assert_eq!(test_cases[0].1.ends_with("sample_1.out"), true);

        std::fs::remove_dir_all(test_dir).unwrap();
    }

    #[test]
    fn collect_test_cases_failed() {
        let test_dir = "collect_test_cases_failed";
        std::fs::create_dir_all(test_dir).unwrap();
        std::fs::write(format!("{}/sample_1.in", test_dir), "input1").unwrap();

        let test_cases = collect_test_cases(Path::new(test_dir)).unwrap();
        assert_eq!(test_cases.len(), 0);

        std::fs::remove_dir_all(test_dir).unwrap();
    }

    #[test]
    fn measure_execution_time_success() {
        let executable_meacure_time = Path::new("./test_executable_meacure_time");
        let input_file = "test_input_1.in";
        std::fs::write(input_file, "test input").unwrap();
        std::fs::write(
            executable_meacure_time,
            r#"#!/bin/bash
            echo "Hello"
            "#,
        )
        .unwrap();

        std::process::Command::new("chmod")
            .arg("+x")
            .arg(executable_meacure_time)
            .status()
            .unwrap();

        let duration =
            measure_execution_time(executable_meacure_time, Path::new(input_file)).unwrap();
        assert!(duration > 0);

        std::fs::remove_file(input_file).unwrap();
        std::fs::remove_file(executable_meacure_time).unwrap();
    }

    #[test]
    fn measure_execution_time_failed() {
        let result = measure_execution_time(
            Path::new("non_existent_executable"),
            Path::new("test_input.in"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn get_execution_output_success() {
        let executable_get_output = Path::new("./test_executable_get_output");
        let input_file = "test_input.in";
        let input_content = "test input data\n";
        std::fs::write(input_file, input_content).unwrap();
        if executable_get_output.exists() {
            std::fs::remove_file(executable_get_output).unwrap();
        }
        std::fs::write(
            executable_get_output,
            r#"#!/bin/bash
            cat
            "#, // 標準入力をそのまま標準出力に返す
        )
        .unwrap();
        std::process::Command::new("chmod")
            .arg("+x")
            .arg(executable_get_output)
            .status()
            .unwrap();

        let output = get_execution_output(executable_get_output, Path::new(input_file)).unwrap();
        assert_eq!(output, input_content);
        std::fs::remove_file(input_file).unwrap();
        std::fs::remove_file(executable_get_output).unwrap();
    }

    #[test]
    fn get_execution_output_failed() {
        let result = get_execution_output(
            Path::new("non_existent_executable"),
            Path::new("test_input.in"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn validate_output_success() {
        let result = validate_output("expected output\n", "expected output\n");
        assert!(result.is_ok());
    }
    #[test]
    fn validate_output_failed() {
        let result = validate_output("expected output\n", "actual output\n");
        assert!(result.is_err());
        let error_message = result.unwrap_err();
        assert!(error_message.contains("Output mismatch"));
    }

    #[test]
    fn return_results_success() {
        let executable = Path::new("./test_return_executable_success");
        let test_dir = "test_return_results";
        let input_file_path = format!("{}/sample_1.in", test_dir);
        let output_file_path = format!("{}/sample_1.out", test_dir);
        let input_file = Path::new(&input_file_path); // ここで借用元を明示
        let output_file = Path::new(&output_file_path);

        std::fs::create_dir_all(test_dir).unwrap();
        std::fs::write(input_file, "test input").unwrap();
        std::fs::write(output_file, "Test Output\n").unwrap();
        std::fs::write(
            executable,
            r#"#!/bin/bash
            echo "Test Output"
            "#,
        )
        .unwrap();
        std::process::Command::new("chmod")
            .arg("+x")
            .arg(executable)
            .status()
            .unwrap();

        let test_cases = vec![(input_file.to_path_buf(), output_file.to_path_buf())];
        let result = return_results(test_cases, executable);
        assert!(result.is_ok());

        std::fs::remove_dir_all(test_dir).unwrap();
        std::fs::remove_file(executable).unwrap();
    }

    #[test]
    fn return_results_failed() {
        let executable = Path::new("./test_return_executable_failed");
        let test_dir = "test_return_results_failure";
        let input_file_path = format!("{}/sample_1.in", test_dir);
        let output_file_path = format!("{}/sample_1.out", test_dir);
        let input_file = Path::new(&input_file_path); // ここで借用元を明示
        let output_file = Path::new(&output_file_path);
        std::fs::create_dir_all(test_dir).unwrap();
        std::fs::write(input_file, "test input").unwrap();
        std::fs::write(output_file, "Expected Output\n").unwrap();
        std::fs::write(
            executable,
            r#"#!/bin/bash
        echo "Actual Output"
        "#,
        )
        .unwrap();
        std::process::Command::new("chmod")
            .arg("+x")
            .arg(executable)
            .status()
            .unwrap();

        let test_cases = vec![(input_file.to_path_buf(), output_file.to_path_buf())];
        let result = return_results(test_cases, executable);
        assert!(result.is_err()); // 処理としてはエラーを返さない

        std::fs::remove_dir_all(test_dir).unwrap();
        std::fs::remove_file(executable).unwrap();
    }
}
