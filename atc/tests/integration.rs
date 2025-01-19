use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn test_integration_with_abc388_a() {
    let test_dir = "./tests/data/abc388";
    let problem_name = "a";
    let input_file = format!("{}/{}/tests/sample_1.in", test_dir, problem_name);

    // サンプル入力を読み込む
    let input_data = fs::read_to_string(&input_file).expect("Failed to read input file");

    // コマンドを実行
    let output = Command::new("cargo")
        .arg("run")
        .arg("test")
        .arg(problem_name)
        .current_dir(test_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            // 標準入力にデータを送信
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(input_data.as_bytes())?;
            }
            child.wait_with_output()
        })
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // デバッグ用ログ
    println!("STDOUT:\n{}", stdout);
    println!("STDERR:\n{}", stderr);

    // 標準出力に 'Test passed' が含まれることを確認
    assert!(
        stdout.contains("Test passed"),
        "Expected 'Test passed' in stdout, but got:\n{}",
        stdout
    );
}
