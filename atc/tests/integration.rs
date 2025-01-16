#[test]
fn test_integration_with_abc388_a() {
    let input_path = "tests/data/abc388/a/sample_1.in";
    let expected_output_path = "tests/data/abc388/a/sample_1.out";

    // テストケースを読み込む
    let input = std::fs::read_to_string(input_path).expect("Failed to read input file");
    let expected_output =
        std::fs::read_to_string(expected_output_path).expect("Failed to read expected output file");

    // `cargo atc test a` をシミュレーション
    let output = std::process::Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--")
        .arg("test")
        .arg("a")
        .output()
        .expect("Failed to execute test command");

    let actual_output = String::from_utf8_lossy(&output.stdout).trim().to_string();

    assert_eq!(actual_output, expected_output, "Test case failed");
}
