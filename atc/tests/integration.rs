use std::{
    env,
    process::{Command, Stdio},
};

#[test]
fn test_integration_with_abc388_a() {
    let test_dir = "tests/data/abc388";
    let problem_name = "a";
    let project_root = env::current_dir().expect("Failed to get current directory");

    // コンパイル
    let _compile_status = Command::new("cargo")
        .arg("build")
        .current_dir(&project_root)
        .status()
        .expect("Failed to compile");

    // testサブコマンド実行
    let binary_path = project_root.join("target/debug/cargo-atc");

    let output = Command::new(binary_path)
        .arg("test")
        .arg(problem_name)
        .current_dir(test_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed with exit code: {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Test passed"),
        "Expected 'Test passed' in stdout, but got:\n{}",
        stdout
    );
    /*
       let stdout = String::from_utf8_lossy(&output.stdout);
       let stderr = String::from_utf8_lossy(&output.stderr);

       // デバッグ用ログ
       println!("DEBUG: STDOUT:\n{}", stdout);
       println!("DEBUG: STDERR:\n{}", stderr);
    */
    // 標準出力に 'Test passed' が含まれることを確認
    /*
    assert!(
        stdout.contains("Test passed"),
        "Expected 'Test passed' in stdout, but got:\n{}",
        stdout
    ); */
}
