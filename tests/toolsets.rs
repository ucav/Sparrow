use sparrow::tools::{known_tool_metadata, metadata_for, surface_allows};

#[test]
fn safe_toolset_does_not_expose_exec_or_edit() {
    let safe: Vec<_> = known_tool_metadata(None)
        .into_iter()
        .filter(|meta| meta.toolset == "safe")
        .collect();
    assert!(
        !safe.is_empty(),
        "safe toolset must contain at least one tool"
    );
    assert!(safe.iter().all(|meta| !meta.exec));
    assert!(safe.iter().all(|meta| !meta.mutates_files));
    assert!(safe.iter().all(|meta| meta.name != "edit"));
}

#[test]
fn python_rpc_is_terminal_not_mcp() {
    let meta = metadata_for("python_rpc", sparrow::event::RiskLevel::Exec);
    assert_eq!(meta.toolset, "terminal");
    assert!(!meta.requires_auth, "python_rpc is local, no auth needed");
    assert!(!meta.network, "python_rpc is local, no network needed");
    assert!(meta.exec);
}

#[test]
fn browser_and_computer_have_separate_risk_profiles() {
    let browser = metadata_for("browser", sparrow::event::RiskLevel::Network);
    assert_eq!(browser.toolset, "web");
    assert!(browser.network);
    assert!(!browser.exec);

    let computer = metadata_for("computer", sparrow::event::RiskLevel::Exec);
    assert_eq!(computer.toolset, "terminal");
    assert!(computer.exec);
    assert!(!surface_allows("gateway", &computer));
}

#[test]
fn todo_is_safe_toolset() {
    let meta = metadata_for("todo", sparrow::event::RiskLevel::ReadOnly);
    assert_eq!(meta.toolset, "safe");
    assert!(!meta.exec);
    assert!(!meta.mutates_files);
    assert!(!meta.network);
}

#[test]
fn debug_profile_can_include_terminal_file_and_web_toolsets() {
    let tools = known_tool_metadata(None);
    assert!(tools.iter().any(|meta| meta.toolset == "terminal"));
    assert!(tools.iter().any(|meta| meta.toolset == "file"));
    assert!(tools.iter().any(|meta| meta.toolset == "web"));
}

#[test]
fn gateway_surface_excludes_dangerous_tools_by_default() {
    let gateway = known_tool_metadata(Some("gateway"));
    assert!(!gateway.iter().any(|meta| meta.exec));
    assert!(!gateway.iter().any(|meta| meta.mutates_files));
    assert!(!gateway.iter().any(|meta| meta.name == "exec"));
    assert!(!gateway.iter().any(|meta| meta.name == "edit"));

    let exec = metadata_for("exec", sparrow::event::RiskLevel::Exec);
    assert!(!surface_allows("gateway", &exec));
}
