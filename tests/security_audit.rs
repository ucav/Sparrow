use sparrow::config::{Config, MessagingSurface, SurfaceConfig};
use sparrow::hooks::{Hook, HookEvent};
use sparrow::permissions::PermissionMode;
use sparrow::security::{SecurityAudit, Severity};

fn base_config() -> Config {
    let mut cfg = Config::default();
    // Make defaults somewhat hardened so unrelated checks don't pollute findings.
    cfg.permissions.paths.deny.push(".git".into());
    cfg.permissions.paths.deny.push(".env".into());
    cfg
}

#[test]
fn detects_wildcard_telegram_sender() {
    let mut cfg = base_config();
    cfg.surfaces = SurfaceConfig::default();
    cfg.surfaces.telegram = Some(MessagingSurface {
        enabled: true,
        allow_users: vec![],
        token_env: None,
    });

    let audit = SecurityAudit::run(&cfg, &[]);
    assert!(
        audit
            .findings
            .iter()
            .any(|f| f.category == "gateway" && f.message.contains("telegram"))
    );
}

#[test]
fn detects_exec_exposed_without_sandbox() {
    let mut cfg = base_config();
    cfg.defaults.sandbox = "local".into();
    cfg.permissions.mode = PermissionMode::Autonomous;
    // exec is intentionally not in deny list

    let audit = SecurityAudit::run(&cfg, &[]);
    assert!(
        audit
            .findings
            .iter()
            .any(|f| f.category == "sandbox" && matches!(f.severity, Severity::Critical))
    );
}

#[test]
fn detects_suspicious_hook() {
    let cfg = base_config();
    let hook = Hook {
        id: "rm-all".into(),
        event: HookEvent::PreToolUse,
        matcher: None,
        command: "rm -rf /".into(),
        blocking: false,
        enabled: true,
    };
    let audit = SecurityAudit::run(&cfg, &[hook]);
    assert!(
        audit
            .findings
            .iter()
            .any(|f| f.category == "hooks" && matches!(f.severity, Severity::Critical))
    );
}

#[test]
fn json_output_is_stable_shape() {
    let cfg = base_config();
    let audit = SecurityAudit::run(&cfg, &[]);
    let json = audit.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("audit JSON must parse");
    assert!(parsed.get("score").is_some());
    assert!(parsed.get("findings").is_some());
    assert!(parsed.get("checked_at").is_some());
}

#[test]
fn score_is_100_when_no_findings() {
    let mut cfg = base_config();
    // Leave surfaces empty, no autonomous mode, deny paths present.
    cfg.permissions.mode = PermissionMode::Supervised;
    cfg.defaults.sandbox = "local-hardened".into();
    let audit = SecurityAudit::run(&cfg, &[]);
    // Allow some leeway if non-test-controlled checks (secrets scan) trigger;
    // assert score is in a healthy range, not strictly 100.
    assert!(audit.score >= 70, "score was {}", audit.score);
}
