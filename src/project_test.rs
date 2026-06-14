use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestRunner {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
}

impl TestRunner {
    pub fn display_command(&self) -> String {
        std::iter::once(self.command.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub fn detect_test_runner(root: &Path) -> Option<TestRunner> {
    if root.join("Cargo.toml").exists() {
        return Some(TestRunner {
            name: "cargo".into(),
            command: "cargo".into(),
            args: vec!["test".into(), "--all-targets".into()],
        });
    }
    if root.join("package.json").exists() {
        return Some(TestRunner {
            name: "npm".into(),
            command: "npm".into(),
            args: vec!["test".into()],
        });
    }
    if root.join("pyproject.toml").exists()
        || root.join("pytest.ini").exists()
        || root.join("setup.cfg").exists()
    {
        return Some(TestRunner {
            name: "pytest".into(),
            command: "python".into(),
            args: vec!["-m".into(), "pytest".into()],
        });
    }
    None
}

/// Ground-truth verification result: did the project's build/test actually pass?
#[derive(Debug, Clone)]
pub struct VerificationOutcome {
    pub passed: bool,
    pub summary: String,
    pub command: String,
}

fn tail(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - max..])
    }
}

/// Run a ground-truth verification in `root`, sandboxed to the workspace: the
/// explicit `verify_command` if set, else an auto-detected test runner. Returns
/// `None` when there is nothing to run (no command and no recognized project),
/// so callers can fall back to a softer review.
///
/// This is the compiler/test-backed signal that turns a verifier from "an LLM's
/// opinion" into "it actually ran" — the strongest verification signal there is.
pub async fn run_verification(
    root: &Path,
    verify_command: Option<&str>,
) -> Option<VerificationOutcome> {
    use crate::sandbox::{Command, Limits, LocalSandbox, Sandbox};
    use std::collections::HashMap;

    let (program, args, display) = match verify_command.map(str::trim).filter(|c| !c.is_empty()) {
        Some(cmd) => {
            let (sh, flag) = if cfg!(windows) {
                ("cmd", "/c")
            } else {
                ("sh", "-c")
            };
            (
                sh.to_string(),
                vec![flag.to_string(), cmd.to_string()],
                cmd.to_string(),
            )
        }
        None => {
            let runner = detect_test_runner(root)?;
            (
                runner.command.clone(),
                runner.args.clone(),
                runner.display_command(),
            )
        }
    };

    let sandbox = LocalSandbox::new(root.to_path_buf());
    let command = Command {
        program,
        args,
        env: HashMap::new(),
        workdir: root.to_path_buf(),
    };
    let limits = Limits {
        timeout_ms: 180_000,
        max_output_bytes: 256 * 1024,
    };
    let result = sandbox.exec(&command, &limits).await.ok()?;

    let mut summary = String::new();
    if !result.stdout.trim().is_empty() {
        summary.push_str(&tail(result.stdout.trim(), 1500));
    }
    if !result.stderr.trim().is_empty() {
        if !summary.is_empty() {
            summary.push('\n');
        }
        summary.push_str(&tail(result.stderr.trim(), 1500));
    }

    Some(VerificationOutcome {
        passed: result.exit_code == 0,
        summary: summary.trim().to_string(),
        command: display,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cargo_runner_first() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("Cargo.toml"), "[package]\nname='x'\n").unwrap();
        let runner = detect_test_runner(temp.path()).unwrap();
        assert_eq!(runner.name, "cargo");
        assert_eq!(runner.display_command(), "cargo test --all-targets");
    }

    #[test]
    fn detects_node_and_python_runners() {
        let node = tempfile::tempdir().unwrap();
        std::fs::write(node.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_test_runner(node.path()).unwrap().name, "npm");

        let py = tempfile::tempdir().unwrap();
        std::fs::write(py.path().join("pyproject.toml"), "[project]\nname='x'\n").unwrap();
        assert_eq!(detect_test_runner(py.path()).unwrap().name, "pytest");
    }

    #[tokio::test]
    async fn verification_runs_and_reports_exit_status() {
        let dir = tempfile::tempdir().unwrap();
        // Explicit verify_command: a passing and a failing shell command.
        let ok = run_verification(dir.path(), Some("exit 0"))
            .await
            .expect("command was run");
        assert!(ok.passed, "exit 0 → passed");

        let bad = run_verification(dir.path(), Some("exit 1"))
            .await
            .expect("command was run");
        assert!(!bad.passed, "exit 1 → failed");

        // Empty workspace, no explicit command → nothing to run.
        assert!(run_verification(dir.path(), None).await.is_none());

        // Whitespace-only command is treated as no command.
        assert!(run_verification(dir.path(), Some("   ")).await.is_none());
    }
}
