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
}
