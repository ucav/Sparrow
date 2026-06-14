//! Smoke for `sparrow do` — the natural-language front door. Uses --dry-run so
//! it resolves intent WITHOUT executing or calling a model. The router's logic
//! is unit-tested in src/nl_router/mod.rs.

use std::process::Command;

fn sparrow() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sparrow"))
}

fn resolve(words: &[&str]) -> String {
    let mut args = vec!["do", "--dry-run"];
    args.extend_from_slice(words);
    let out = sparrow().args(&args).output().expect("run sparrow");
    assert!(out.status.success(), "dry-run must exit 0");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn natural_language_resolves_to_the_right_command() {
    assert!(resolve(&["corrige", "le", "build"]).contains("sparrow fix"));
    assert!(resolve(&["montre", "la", "console"]).contains("sparrow console"));
    assert!(resolve(&["quels", "modèles", "sont", "dispo"]).contains("sparrow model --list"));
    // Unknown phrasing falls back to the general agent, never an error.
    assert!(resolve(&["zork", "frobnicate", "xyz"]).contains("sparrow run"));
}

#[test]
fn empty_request_is_rejected() {
    let out = sparrow().args(["do", ""]).output().expect("run sparrow");
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .to_lowercase()
            .contains("usage")
    );
}
