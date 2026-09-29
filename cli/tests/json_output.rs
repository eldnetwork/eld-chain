use std::process::Command;

#[test]
fn wallet_list_json_is_valid_and_warning_stays_on_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let wallets = dir.path().join("wallets.json");
    std::fs::write(&wallets, "[]").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_eld-cli"))
        .args([
            "--output",
            "json",
            "--wallets",
            wallets.to_str().unwrap(),
            "wallet",
            "list",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    let parsed: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|err| {
        panic!("stdout is not JSON: {err}\n{stdout}");
    });
    assert!(parsed.get("wallets").and_then(|v| v.as_array()).is_some());
    assert!(!stdout.contains("Warning"), "{stdout}");
    assert!(!stdout.contains("private"), "{stdout}");
    assert!(stderr.contains("unencrypted Ed25519 keys"), "{stderr}");
}
