use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

pub fn execute(problem_name: &str) -> Result<(), Box<dyn Error>> {
    let dir = find_problem_directory(problem_name)?;
    let _ = compile(&dir)?;
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

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;

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
        let executable = Path::new("./test_executable");
        let input_file = "test_input_1.in";
        std::fs::write(input_file, "test input").unwrap();
        std::fs::write(
            executable,
            r#"#!/bin/bash
            echo "Hello"
            "#,
        )
        .unwrap();

        std::process::Command::new("chmod")
            .arg("+x")
            .arg(executable)
            .status()
            .unwrap();

        let duration = measure_execution_time(executable, Path::new(input_file)).unwrap();
        assert!(duration > 0);

        std::fs::remove_file(input_file).unwrap();
        std::fs::remove_file(executable).unwrap();
    }

    #[test]
    fn measure_execution_time_failed() {
        let result = measure_execution_time(
            Path::new("non_existent_executable"),
            Path::new("test_input.in"),
        );
        assert!(result.is_err());
    }
}
