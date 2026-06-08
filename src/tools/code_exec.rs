//! Code execution in sandbox — runs code with timeout.

use std::process::Command;
use std::time::Duration;

pub fn execute_code(code: &str, language: &str, timeout_secs: u64) -> anyhow::Result<ExecResult> {
    let tmpdir = std::env::temp_dir().join("sparrow_exec");
    std::fs::create_dir_all(&tmpdir)?;

    let (ext, runner, args): (&str, &str, Vec<&str>) = match language {
        "python" | "py" => ("py", "python3", vec![]),
        "javascript" | "js" => ("js", "node", vec![]),
        "bash" | "sh" => ("sh", "bash", vec![]),
        "ruby" | "rb" => ("rb", "ruby", vec![]),
        _ => anyhow::bail!("Unsupported: {language}. Use python, js, bash, or ruby."),
    };

    let file = tmpdir.join(format!("code_{}.{}", uuid::Uuid::new_v4(), ext));
    std::fs::write(&file, code)?;

    let output = Command::new("timeout")
        .arg(timeout_secs.to_string())
        .arg(runner)
        .args(&args)
        .arg(&file)
        .output()?;

    Ok(ExecResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
        language: language.to_string(),
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub language: String,
}
