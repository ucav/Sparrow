use clap::Parser;
use sparrow::cli::{Cli, Commands};
use sparrow::config::{Config, ConfigStore, FsConfigStore, human_config_header};

#[test]
fn launch_has_explicit_pro_escape_hatch() {
    let cli = Cli::try_parse_from(["sparrow", "launch", "--pro", "--port", "9444"]).unwrap();
    match cli.command {
        Some(Commands::Launch { port, tui, pro }) => {
            assert_eq!(port, 9444);
            assert!(!tui);
            assert!(pro);
        }
        _ => panic!("expected launch command"),
    }
}

#[test]
fn saved_config_starts_with_human_comments() {
    let tmp = tempfile::tempdir().unwrap();
    let store = FsConfigStore::new(tmp.path().to_path_buf());
    store.save(&Config::default()).unwrap();

    let text = std::fs::read_to_string(tmp.path().join("config.toml")).unwrap();
    assert!(text.starts_with(human_config_header()));
    assert!(text.contains("Mode simple par défaut"));
    assert!(store.load().is_ok(), "comments must keep TOML loadable");
}

#[test]
fn focus_cockpit_contract_is_present() {
    let html = std::fs::read_to_string("console.html").expect("console.html must exist");

    for marker in [
        "sparrow-view-mode",
        "focusModeBtn",
        "cockpitModeBtn",
        "focusOkBtn",
        "focusUndoBtn",
        "focusExplainBtn",
        "micBtn",
        "sparrow-focus-tour-done",
        "html[data-view=\"focus\"]",
        "Alt+F",
    ] {
        assert!(
            html.contains(marker),
            "missing v0.9 Focus marker `{marker}`"
        );
    }

    assert!(html.contains(">OK</button>"));
    assert!(html.contains(">Undo</button>"));
    assert!(html.contains(">Explain</button>"));
    assert!(html.contains("aria-label=\"Dictate with microphone\""));
    assert!(!html.contains("Ton point de départ"));
}

#[test]
fn v0_9_mockup_and_a11y_gate_exist() {
    let mockup =
        std::fs::read_to_string("sparrow-cockpit-v0.9.0-mockup.html").expect("mockup missing");
    assert!(mockup.contains("Focus"));
    assert!(mockup.contains("Cockpit"));
    assert!(mockup.contains(">OK</button>"));
    assert!(mockup.contains(">Undo</button>"));
    assert!(mockup.contains(">Explain</button>"));
    assert!(mockup.contains("Sparrow is ready"));

    let script = std::fs::read_to_string("scripts/audit-a11y.mjs").expect("audit script missing");
    for marker in [
        "Focus mode is default",
        "Alt+F toggles view",
        "AA contrast",
        "No JavaScript runtime errors",
    ] {
        assert!(script.contains(marker), "missing audit marker `{marker}`");
    }

    let package = std::fs::read_to_string("package.json").expect("package.json missing");
    assert!(package.contains("\"a11y:console\""));
}
