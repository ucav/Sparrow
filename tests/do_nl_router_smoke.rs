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
    assert!(
        !out.status.success(),
        "empty request must not silently succeed"
    );
    let err = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(err.contains("langage naturel"), "stderr: {err}");
}

/// The real goal: BARE text with NO command word. `sparrow corrige le build`
/// must route to `fix`. SPARROW_NL_PREVIEW makes it resolve without executing.
fn bare(words: &[&str]) -> String {
    let out = sparrow()
        .args(words)
        .env("SPARROW_NL_PREVIEW", "1")
        .output()
        .expect("run sparrow");
    assert!(out.status.success(), "bare text must not error: {words:?}");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn bare_text_routes_with_no_command_word() {
    // Unknown first word → external_subcommand → router.
    assert!(bare(&["corrige", "le", "build"]).contains("sparrow fix"));
    // First word collides with a no-payload alias (`montre`→console) → the
    // try_parse fallback re-routes it as natural language instead of erroring.
    assert!(bare(&["montre", "la", "console"]).contains("sparrow console"));
    assert!(bare(&["liste", "les", "points", "de", "sauvegarde"]).contains("checkpoint list"));
    // Real flags are never hijacked.
    let v = sparrow().arg("--version").output().expect("run");
    assert!(v.status.success());
    assert!(String::from_utf8_lossy(&v.stdout).contains("sparrow"));
}
