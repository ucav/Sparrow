use std::collections::HashMap;
use std::path::PathBuf;

use sparrow::sandbox::backends::{
    DaytonaSandbox, ModalSandbox, SingularitySandbox, VercelSandbox, WorktreeSandbox,
};
use sparrow::sandbox::{
    Command, FsNetPolicy, Limits, LocalSandbox, Sandbox, default_denied_paths, path_is_denied,
};

fn limits() -> Limits {
    Limits {
        timeout_ms: 5_000,
        max_output_bytes: 65_536,
    }
}

#[test]
fn path_is_denied_matches_components() {
    let denied = default_denied_paths();
    assert!(path_is_denied(&PathBuf::from("repo/.git/config"), &denied));
    assert!(path_is_denied(&PathBuf::from(".env"), &denied));
    assert!(path_is_denied(
        &PathBuf::from("home/user/.ssh/id_rsa"),
        &denied
    ));
    assert!(!path_is_denied(&PathBuf::from("src/main.rs"), &denied));
    assert!(!path_is_denied(
        &PathBuf::from("docs/git-notes.md"),
        &denied
    ));
}

#[tokio::test]
async fn workdir_outside_root_is_rejected() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path().to_path_buf();
    let sandbox = LocalSandbox::new(root);

    // Workdir is the parent of root → escapes
    let parent = tmp.path().parent().unwrap().to_path_buf();
    let cmd = Command {
        program: if cfg!(windows) {
            "cmd".into()
        } else {
            "true".into()
        },
        args: vec![],
        env: HashMap::new(),
        workdir: parent,
    };
    let res = sandbox.exec(&cmd, &limits()).await;
    assert!(
        res.is_err(),
        "expected workdir-escape error, got {:?}",
        res.ok().map(|r| r.exit_code)
    );
}

#[tokio::test]
async fn arg_referring_to_protected_path_is_rejected() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path().to_path_buf();
    let policy = FsNetPolicy {
        allowed_paths: vec![root.clone()],
        allow_network: false,
        denied_paths: default_denied_paths(),
        env_allowlist: Vec::new(),
    };
    let sandbox = LocalSandbox::new(root.clone()).with_policy(policy);

    let cmd = Command {
        program: "echo".into(),
        args: vec!["read".into(), ".git/config".into()],
        env: HashMap::new(),
        workdir: root,
    };
    let res = sandbox.exec(&cmd, &limits()).await;
    assert!(res.is_err(), "protected path arg should be rejected");
}

#[tokio::test]
async fn env_allowlist_filters_environment() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path().to_path_buf();

    let policy = FsNetPolicy {
        allowed_paths: vec![root.clone()],
        allow_network: true,
        denied_paths: vec![], // disable for this check
        env_allowlist: vec!["KEEP".into()],
    };
    let sandbox = LocalSandbox::new(root.clone()).with_policy(policy);

    let mut env = HashMap::new();
    env.insert("KEEP".into(), "yes".into());
    env.insert("STRIP".into(), "no".into());

    // On Windows, `cmd /c echo %KEEP% %STRIP%` would render literal `%STRIP%`
    // if STRIP is unset; on POSIX we use `sh -c 'echo $KEEP $STRIP'`.
    let (program, args) = if cfg!(windows) {
        (
            "cmd".to_string(),
            vec!["/c".into(), "echo %KEEP%-%STRIP%".into()],
        )
    } else {
        (
            "sh".to_string(),
            vec!["-c".into(), "echo \"$KEEP-$STRIP\"".into()],
        )
    };

    let cmd = Command {
        program,
        args,
        env,
        workdir: root,
    };
    let res = sandbox.exec(&cmd, &limits()).await.expect("exec ok");
    let out = res.stdout.trim().to_string();
    assert!(out.contains("yes"), "KEEP must be forwarded, got {:?}", out);
    assert!(
        !out.contains("no"),
        "STRIP must be filtered out, got {:?}",
        out
    );
}

#[tokio::test]
async fn modal_returns_honest_error_when_cli_missing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path().to_path_buf();
    let sandbox = ModalSandbox::new(root.clone());
    let cmd = Command {
        program: "echo".into(),
        args: vec!["hi".into()],
        env: HashMap::new(),
        workdir: root,
    };
    let res = sandbox.exec(&cmd, &limits()).await.expect("exec call");
    // Modal CLI is not installed in CI → must return exit 127 and a clear stderr.
    // (If a developer happens to have `modal` installed this test will still pass
    // because we only assert a property when the CLI is absent.)
    let modal_present = std::process::Command::new("modal")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !modal_present {
        assert_eq!(res.exit_code, 127);
        assert!(
            res.stderr.contains("not found") || res.stderr.contains("unavailable"),
            "stderr must be clear, got: {:?}",
            res.stderr
        );
    }
}

#[tokio::test]
async fn daytona_vercel_singularity_also_honest_when_missing() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path().to_path_buf();
    for sb in [
        Box::new(DaytonaSandbox::new(root.clone())) as Box<dyn Sandbox>,
        Box::new(VercelSandbox::new(root.clone())),
        Box::new(SingularitySandbox::new(root.clone())),
    ] {
        let cmd = Command {
            program: "echo".into(),
            args: vec!["hi".into()],
            env: HashMap::new(),
            workdir: root.clone(),
        };
        let res = sb.exec(&cmd, &limits()).await.expect("exec call");
        // We never want a fake success when the CLI is missing.
        if res.exit_code != 0 {
            assert!(
                !res.stderr.is_empty(),
                "missing-CLI failures must report a reason"
            );
        }
    }
}

#[test]
fn worktree_sandbox_rejects_non_git_root() {
    let tmp = tempfile::tempdir().expect("tmp");
    let parent = tempfile::tempdir().expect("tmp2");
    let res = WorktreeSandbox::create(tmp.path(), parent.path(), "sparrow-test");
    let err = match res {
        Ok(_) => panic!("non-git root must fail clearly"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("git repo"), "error must explain why: {}", err);
}
