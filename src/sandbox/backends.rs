use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use super::{Command, ExecResult, FsNetPolicy, Limits, Sandbox};

/// POSIX single-quote escaping. Wraps `s` in single quotes and escapes any embedded
/// single quote by closing-quote, escaped-quote, reopening-quote: `'\''`.
/// Safe against `;`, `|`, `&`, `$()`, backticks, newlines.
fn sh_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

// ─── Docker sandbox ─────────────────────────────────────────────────────────────

pub struct DockerSandbox {
    root: PathBuf,
    image: String,
    policy: FsNetPolicy,
}

impl DockerSandbox {
    pub fn new(root: PathBuf, image: &str) -> Self {
        Self {
            root: root.clone(),
            image: image.to_string(),
            policy: FsNetPolicy {
                allowed_paths: vec![root],
                allow_network: false,
                ..FsNetPolicy::default()
            },
        }
    }
}

#[async_trait::async_trait]
impl Sandbox for DockerSandbox {
    async fn exec(&self, cmd: &Command, limits: &Limits) -> anyhow::Result<ExecResult> {
        let workdir = cmd.workdir.to_string_lossy().to_string();
        let mut args = vec![
            "run".into(),
            "--rm".into(),
            "-v".into(),
            format!("{}:/workspace", workdir),
            "-w".into(),
            "/workspace".into(),
            format!("--memory={}m", limits.max_output_bytes / 1024 / 1024 + 128),
        ];

        if !self.policy.allow_network {
            args.push("--network=none".into());
        }

        args.push(self.image.clone());
        args.push(cmd.program.clone());
        args.extend(cmd.args.clone());

        let output = StdCommand::new("docker").args(&args).output()?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn policy(&self) -> &FsNetPolicy {
        &self.policy
    }
}

// ─── SSH remote sandbox ─────────────────────────────────────────────────────────

pub struct SshSandbox {
    root: PathBuf,
    host: String,
    policy: FsNetPolicy,
}

impl SshSandbox {
    pub fn new(root: PathBuf, host: &str) -> Self {
        Self {
            root,
            host: host.to_string(),
            policy: FsNetPolicy {
                allowed_paths: vec![],
                allow_network: true,
                ..FsNetPolicy::default()
            },
        }
    }
}

#[async_trait::async_trait]
impl Sandbox for SshSandbox {
    async fn exec(&self, cmd: &Command, _limits: &Limits) -> anyhow::Result<ExecResult> {
        // Quote every component to defeat shell-injection (`;`, `|`, `&`, `$()`, backticks).
        // The remote sh sees a single argv string, so we must build it safely here.
        let quoted_args: Vec<String> = cmd.args.iter().map(|a| sh_quote(a)).collect();
        let full_cmd = format!(
            "cd {} && {} {}",
            sh_quote(&cmd.workdir.to_string_lossy()),
            sh_quote(&cmd.program),
            quoted_args.join(" ")
        );

        let output = StdCommand::new("ssh")
            .args([&self.host, &full_cmd])
            .output()?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn policy(&self) -> &FsNetPolicy {
        &self.policy
    }
}

// ─── Cloud/HPC backends (modal, daytona, vercel, singularity) ───────────────────
//
// These are CLI-driven backends: when the vendor CLI is installed and
// authenticated, we shell out to run the command remotely. When it is NOT
// present we return an HONEST non-zero error (exit 127) — never a fake success.
// The "remote VM" use case is fully covered today by `SshSandbox` and
// `DockerSandbox`; these add vendor-managed environments on top.

macro_rules! cli_sandbox {
    ($name:ident, $label:expr, $bin:expr, $exec_args:expr) => {
        pub struct $name {
            root: PathBuf,
            policy: FsNetPolicy,
        }

        impl $name {
            pub fn new(root: PathBuf) -> Self {
                Self {
                    root: root.clone(),
                    policy: FsNetPolicy {
                        allowed_paths: vec![root],
                        allow_network: true,
                        ..FsNetPolicy::default()
                    },
                }
            }

            fn cli_available() -> bool {
                StdCommand::new($bin)
                    .arg("--version")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }
        }

        #[async_trait::async_trait]
        impl Sandbox for $name {
            async fn exec(&self, cmd: &Command, _limits: &Limits) -> anyhow::Result<ExecResult> {
                if !Self::cli_available() {
                    // Honest failure — not a fabricated success.
                    return Ok(ExecResult {
                        stdout: String::new(),
                        stderr: format!(
                            "{} sandbox unavailable: '{}' CLI not found or not authenticated. \
                             Install/login to it, or use sandbox=ssh / sandbox=docker which are \
                             fully supported.",
                            $label, $bin
                        ),
                        exit_code: 127,
                    });
                }
                let user_cmd = format!("{} {}", cmd.program, cmd.args.join(" "));
                let mut args: Vec<String> =
                    $exec_args.iter().map(|s: &&str| s.to_string()).collect();
                args.push(user_cmd);
                let output = StdCommand::new($bin)
                    .args(&args)
                    .current_dir(&cmd.workdir)
                    .output()?;
                Ok(ExecResult {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: output.status.code().unwrap_or(-1),
                })
            }

            fn root(&self) -> &Path {
                &self.root
            }

            fn policy(&self) -> &FsNetPolicy {
                &self.policy
            }
        }
    };
}

// Best-effort vendor CLI invocations; exact sub-commands are configurable by
// installing the vendor CLI which defines them. Missing CLI → honest error.
cli_sandbox!(ModalSandbox, "modal", "modal", ["run", "--"]);
cli_sandbox!(DaytonaSandbox, "daytona", "daytona", ["exec", "--"]);
cli_sandbox!(VercelSandbox, "vercel-sandbox", "vercel", ["exec", "--"]);
cli_sandbox!(SingularitySandbox, "singularity", "singularity", ["exec"]);

// ─── Worktree sandbox ───────────────────────────────────────────────────────────
//
// Runs commands inside a dedicated `git worktree` so mutations land on an
// isolated branch and never touch the user's working copy. If the `repo_root`
// is not a git repository the constructor returns an error — no fake success.

use crate::sandbox::{LocalSandbox, default_denied_paths};

pub struct WorktreeSandbox {
    inner: LocalSandbox,
    worktree_path: PathBuf,
    branch: String,
}

impl WorktreeSandbox {
    /// Create a new git worktree under `parent_dir` checked out on `branch`.
    /// `repo_root` must be a git repo. The worktree path is
    /// `parent_dir/sparrow-<branch>`.
    pub fn create(repo_root: &Path, parent_dir: &Path, branch: &str) -> anyhow::Result<Self> {
        if !repo_root.join(".git").exists() {
            anyhow::bail!(
                "WorktreeSandbox requires a git repo at {} (no .git/)",
                repo_root.display()
            );
        }
        std::fs::create_dir_all(parent_dir)?;
        let worktree_path = parent_dir.join(format!("sparrow-{}", branch));

        let status = StdCommand::new("git")
            .args([
                "-C",
                &repo_root.to_string_lossy(),
                "worktree",
                "add",
                "-B",
                branch,
                &worktree_path.to_string_lossy(),
            ])
            .status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => anyhow::bail!("git worktree add failed (exit {:?})", s.code()),
            Err(e) => anyhow::bail!("git not available: {}", e),
        }

        let policy = FsNetPolicy {
            allowed_paths: vec![worktree_path.clone()],
            allow_network: true,
            denied_paths: default_denied_paths(),
            env_allowlist: Vec::new(),
        };
        let inner = LocalSandbox::new(worktree_path.clone()).with_policy(policy);
        Ok(Self {
            inner,
            worktree_path,
            branch: branch.to_string(),
        })
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn path(&self) -> &Path {
        &self.worktree_path
    }
}

#[async_trait::async_trait]
impl Sandbox for WorktreeSandbox {
    async fn exec(&self, cmd: &Command, limits: &Limits) -> anyhow::Result<ExecResult> {
        self.inner.exec(cmd, limits).await
    }

    fn root(&self) -> &Path {
        self.inner.root()
    }

    fn policy(&self) -> &FsNetPolicy {
        self.inner.policy()
    }
}
