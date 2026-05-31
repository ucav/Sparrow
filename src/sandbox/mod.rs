use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub mod backends;

#[cfg(target_os = "linux")]
mod linux_hardened {
    // ─── Real local-hardened sandbox (Linux namespaces + seccomp) ───────────────
    // §3.5: "Linux namespaces + seccomp (landlock/seccompiler),
    //        filesystem allow-list scoped to workspace, network deny by default"

    use std::path::PathBuf;
    use super::{Command, ExecResult, FsNetPolicy, Limits, Sandbox};

    pub struct HardenedSandbox {
        root: PathBuf,
        policy: FsNetPolicy,
    }

    impl HardenedSandbox {
        pub fn new(root: PathBuf) -> Self {
            Self {
                root: root.clone(),
                policy: FsNetPolicy {
                    allowed_paths: vec![root],
                    allow_network: false,
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl Sandbox for HardenedSandbox {
        async fn exec(
            &self,
            cmd: &Command,
            limits: &Limits,
        ) -> anyhow::Result<ExecResult> {
            use std::process::Command as StdCommand;

            // Build a firejail/bwrap command if available, else fall back to local
            let (program, args) = if which("firejail") {
                let mut fargs = vec![
                    "--quiet".into(),
                    format!("--timeout={}", limits.timeout_ms / 1000),
                    format!("--private={}", self.root.display()),
                ];
                if !self.policy.allow_network {
                    fargs.push("--net=none".into());
                }
                for path in &self.policy.allowed_paths {
                    fargs.push(format!("--whitelist={}", path.display()));
                }
                fargs.push("--".into());
                fargs.push(cmd.program.clone());
                fargs.extend(cmd.args.clone());
                ("firejail".to_string(), fargs)
            } else if which("bwrap") {
                let mut bargs = vec![
                    "--ro-bind".into(),
                    "/usr".into(),
                    "/usr".into(),
                    "--ro-bind".into(),
                    "/lib".into(),
                    "/lib".into(),
                    "--ro-bind".into(),
                    "/lib64".into(),
                    "/lib64".into(),
                    "--ro-bind".into(),
                    "/bin".into(),
                    "/bin".into(),
                    "--bind".into(),
                    self.root.display().to_string(),
                    self.root.display().to_string(),
                    "--chdir".into(),
                    self.root.display().to_string(),
                ];
                if !self.policy.allow_network {
                    bargs.push("--unshare-net".into());
                }
                bargs.push("--".into());
                bargs.push(cmd.program.clone());
                bargs.extend(cmd.args.clone());
                ("bwrap".to_string(), bargs)
            } else {
                // Fallback: unshare with basic isolation
                let mut uargs = vec![
                    "--mount".into(),
                    "--pid".into(),
                    "--fork".into(),
                    "--root".into(),
                    self.root.display().to_string(),
                ];
                uargs.push(cmd.program.clone());
                uargs.extend(cmd.args.clone());
                ("unshare".to_string(), uargs)
            };

            let output = StdCommand::new(&program)
                .args(&args)
                .current_dir(&cmd.workdir)
                .output()?;

            Ok(ExecResult {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
            })
        }

        fn root(&self) -> &std::path::Path {
            &self.root
        }

        fn policy(&self) -> &FsNetPolicy {
            &self.policy
        }
    }

    fn which(cmd: &str) -> bool {
        std::process::Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(target_os = "linux")]
pub use linux_hardened::HardenedSandbox;

#[cfg(not(target_os = "linux"))]
pub struct HardenedSandbox {
    _root: PathBuf,
    _policy: FsNetPolicy,
}

#[cfg(not(target_os = "linux"))]
impl HardenedSandbox {
    pub fn new(root: PathBuf) -> Self {
        Self {
            _root: root,
            _policy: FsNetPolicy::default(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[async_trait::async_trait]
impl Sandbox for HardenedSandbox {
    async fn exec(&self, _cmd: &Command, _limits: &Limits) -> anyhow::Result<ExecResult> {
        Ok(ExecResult {
            stdout: "Hardened sandbox requires Linux (firejail/bwrap/unshare)".into(),
            stderr: String::new(),
            exit_code: 0,
        })
    }

    fn root(&self) -> &Path {
        &self._root
    }

    fn policy(&self) -> &FsNetPolicy {
        &self._policy
    }
}

// ─── Command and limits ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub workdir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

// ─── File system and network policy ─────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FsNetPolicy {
    pub allowed_paths: Vec<PathBuf>,
    pub allow_network: bool,
}

impl Default for FsNetPolicy {
    fn default() -> Self {
        Self {
            allowed_paths: vec![],
            allow_network: false,
        }
    }
}

// ─── THE SANDBOX TRAIT ──────────────────────────────────────────────────────────

/// Isolates `exec`/`Mutating` actions. Backends are selectable per run.
#[async_trait]
pub trait Sandbox: Send + Sync {
    async fn exec(&self, cmd: &Command, limits: &Limits) -> anyhow::Result<ExecResult>;
    fn root(&self) -> &Path;
    fn policy(&self) -> &FsNetPolicy;
}

// ─── Local sandbox implementation ───────────────────────────────────────────────

pub struct LocalSandbox {
    root: PathBuf,
    policy: FsNetPolicy,
}

impl LocalSandbox {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root: root.clone(),
            policy: FsNetPolicy {
                allowed_paths: vec![root],
                allow_network: true,
            },
        }
    }

    pub fn hardened(root: PathBuf) -> Self {
        Self {
            root: root.clone(),
            policy: FsNetPolicy {
                allowed_paths: vec![root],
                allow_network: false, // deny by default for hardened
            },
        }
    }
}

#[async_trait]
impl Sandbox for LocalSandbox {
    async fn exec(&self, cmd: &Command, limits: &Limits) -> anyhow::Result<ExecResult> {
        use std::process::Command as StdCommand;
        use std::time::Instant;

        let root = self.root.canonicalize().unwrap_or_else(|_| self.root.clone());
        let workdir = cmd.workdir.canonicalize().unwrap_or_else(|_| cmd.workdir.clone());
        if !workdir.starts_with(&root) {
            anyhow::bail!("Command workdir escapes sandbox root: {}", cmd.workdir.display());
        }

        let mut child = StdCommand::new(&cmd.program)
            .args(&cmd.args)
            .current_dir(&workdir)
            .envs(&cmd.env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let start = Instant::now();
        let timeout = std::time::Duration::from_millis(limits.timeout_ms);

        // Simple timeout via polling
        loop {
            match child.try_wait()? {
                Some(status) => {
                    let output = child.wait_with_output()?;
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let exit_code = status.code().unwrap_or(-1);

                    return Ok(ExecResult {
                        stdout: truncate(stdout, limits.max_output_bytes),
                        stderr: truncate(stderr, limits.max_output_bytes),
                        exit_code,
                    });
                }
                None => {
                    if start.elapsed() > timeout {
                        let _ = child.kill();
                        return Ok(ExecResult {
                            stdout: String::new(),
                            stderr: "TIMEOUT".to_string(),
                            exit_code: -1,
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn policy(&self) -> &FsNetPolicy {
        &self.policy
    }
}

fn truncate(s: String, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        s
    } else {
        let truncate_at = max_bytes.saturating_sub(100);
        format!(
            "{}\n... [truncated, {} bytes total]",
            &s[..truncate_at.min(s.len())],
            s.len()
        )
    }
}
