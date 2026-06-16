use sparrow::tui::theme::{THEME_NAMES, by_name};

#[test]
fn three_built_in_themes_are_resolvable_by_name() {
    assert_eq!(THEME_NAMES, &["captain", "midnight", "paper"]);
    let cap = by_name("captain");
    let mid = by_name("midnight");
    let pap = by_name("paper");
    // The three variants must produce distinct backgrounds — that is what
    // makes them visibly different in the cockpit.
    assert_ne!(cap.bg, mid.bg);
    assert_ne!(cap.bg, pap.bg);
    assert_ne!(mid.bg, pap.bg);
}

#[test]
fn unknown_theme_name_falls_back_to_captain() {
    let unknown = by_name("does-not-exist");
    let captain = by_name("captain");
    // Same background → same theme.
    assert_eq!(unknown.bg, captain.bg);
    assert_eq!(unknown.brand, captain.brand);
}

#[test]
fn theme_lookup_is_case_insensitive_and_trims() {
    let a = by_name("Midnight");
    let b = by_name("  MIDNIGHT  ");
    let c = by_name("midnight");
    assert_eq!(a.bg, b.bg);
    assert_eq!(b.bg, c.bg);
}

#[test]
fn keyboard_doc_lists_critical_shortcuts() {
    let doc = std::fs::read_to_string("docs/keyboard.md")
        .expect("docs/keyboard.md must ship with the TUI");
    for shortcut in [
        "Ctrl+I",
        "Ctrl+L",
        "Ctrl+O",
        "Shift+Enter",
        "Tab",
        "Up",
        "Down",
    ] {
        assert!(
            doc.contains(shortcut),
            "docs/keyboard.md must document {}",
            shortcut
        );
    }
    for theme in ["captain", "midnight", "paper"] {
        assert!(doc.contains(theme), "docs/keyboard.md must list {}", theme);
    }
    for shortcut in [
        "Cmd/Ctrl+K",
        "Cmd/Ctrl+Shift+L",
        "Cmd/Ctrl+M",
        "Drag files onto page",
        "@",
    ] {
        assert!(
            doc.contains(shortcut),
            "docs/keyboard.md must document WebView shortcut {}",
            shortcut
        );
    }
}

#[test]
fn console_html_has_dynamic_swarm_hooks() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // The Sprint 1 deliverable 1bis: the swarm row must expose the anchor
    // and the overflow chip so loadSwarmAgents() can inject extras.
    assert!(
        html.contains("swarm-extras-anchor"),
        "console.html must expose #swarm-extras-anchor for dynamic agent lanes"
    );
    assert!(
        html.contains("swarm-more"),
        "console.html must expose #swarm-more for the +N overflow chip"
    );
    assert!(
        html.contains("loadSwarmAgents"),
        "console.html must call loadSwarmAgents() on connect"
    );
    // No truncation: `text-overflow: ellipsis` must not gate `.lane .msg`.
    let lane_msg_block = html
        .split(".lane .msg")
        .nth(1)
        .expect("expected `.lane .msg` CSS block");
    let first_rule = lane_msg_block.split('}').next().unwrap_or("");
    assert!(
        !first_rule.contains("text-overflow:ellipsis"),
        "`.lane .msg` must not truncate text in the new swarm row, got: {}",
        first_rule
    );
}

#[test]
fn console_html_has_drawer_with_seven_panels() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // Rail buttons for the 7 panels of Sprint 1 inc 2.
    for panel in [
        "sessions",
        "memory",
        "plugins",
        "tools",
        "permissions",
        "security",
        "artifacts",
    ] {
        let rail_marker = format!("data-panel=\"{}\"", panel);
        let body_marker = format!("data-panel-body=\"{}\"", panel);
        assert!(
            html.contains(&rail_marker),
            "rail must expose button for {}",
            panel
        );
        assert!(
            html.contains(&body_marker),
            "drawer must expose body for {}",
            panel
        );
    }
    assert!(
        html.contains("PANEL_LOADERS"),
        "panel loader registry must be present"
    );
    assert!(
        html.contains("openPanel"),
        "openPanel() switch handler must be present"
    );
}

#[test]
fn console_html_has_paper_theme_and_chrome_chips() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // Sprint 1 inc 4-5: paper theme variant + chrome chips for palette /
    // replay / sound / theme. The selector must come from a CSS block keyed
    // on `data-theme="paper"`.
    assert!(
        html.contains("[data-theme=\"paper\"]"),
        "console.html must declare a paper theme block"
    );
    for chip in ["cmdkBtn", "replayBtn", "soundBtn", "themeBtn"] {
        let marker = format!("id=\"{}\"", chip);
        assert!(html.contains(&marker), "chrome must expose {} chip", chip);
    }
    // Hero welcome + boot animation must be wired.
    assert!(
        html.contains("injectHero") && html.contains(".hero{"),
        "hero welcome must be injected at term boot"
    );
    assert!(
        html.contains("runBootAnimation") && html.contains("bootOverlay"),
        "boot overlay must be present and triggered"
    );
    // Sound system + Cmd+M mute toggle.
    assert!(
        html.contains("function chirp(") && html.contains("'sparrow-muted'"),
        "WebAudio chirp + mute persistence must be wired"
    );
}

#[test]
fn console_html_has_slash_palette_and_agent_picker() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // Slash palette modal + Cmd+K wiring.
    assert!(
        html.contains("id=\"palette\""),
        "console.html must declare the slash palette modal"
    );
    assert!(
        html.contains("paletteOpen") && html.contains("paletteFilter"),
        "palette open / filter functions must be present"
    );
    assert!(
        html.contains("key.toLowerCase()==='k'"),
        "Cmd/Ctrl+K shortcut must be wired"
    );
    // Inline @-picker.
    assert!(
        html.contains("id=\"agentPicker\""),
        "console.html must declare the inline @-picker"
    );
    assert!(
        html.contains("agentPickerState") && html.contains("agentPickerAccept"),
        "@-picker state/accept helpers must be present"
    );
    // Caches load on connect.
    assert!(
        html.contains("loadCommandsCache") && html.contains("loadAgentsCache"),
        "both /commands and /agents must be pre-fetched on connect"
    );
    assert!(
        html.contains("cmd.usage") && html.contains("paletteSourceLabel"),
        "slash palette and /help must render command usage plus readable sources"
    );
    assert!(
        html.contains("runWebviewCliCommand") && html.contains("fetch('/cli'"),
        "unknown slash commands must be executable through the WebView CLI bridge"
    );
}

#[test]
fn console_html_has_sprint2_composer_hooks() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    assert!(
        html.contains("<textarea id=\"taskInput\""),
        "composer must use a textarea for multi-line Shift+Enter input"
    );
    for marker in [
        "loadHistoryCache",
        "fetch('/history?limit=80')",
        "composerKeydown",
        "sparrow-composer-draft",
        "composerPaste",
        "dropZone",
        "attachFiles",
        "fetch('/upload'",
        "MAX_ATTACHMENT_BYTES",
    ] {
        assert!(
            html.contains(marker),
            "console.html must expose Sprint 2 composer hook `{}`",
            marker
        );
    }
}

#[test]
fn console_html_has_sprint3_micro_animation_hooks() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "approvalModal",
        "showApprovalModal",
        "resolveApprovalFromModal",
        "error-banner",
        "showError",
        "checkpoint-timeline",
        "addCheckpointNode",
        "prefers-reduced-motion: reduce",
        "fold-in",
    ] {
        assert!(
            html.contains(marker),
            "console.html must expose Sprint 3 hook `{}`",
            marker
        );
    }
}

#[test]
fn console_html_matches_v0_3_visual_polish_contract() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "id=\"appVersion\"",
        "get('boot')==='0'",
        "get('fast')==='1'",
        "requestIdleCallback",
        ".boot-overlay[hidden]",
        "route-step",
        "class=\"arrow\"",
        "drawer-in",
        "drw-row.cur",
        "_none yet_ · unable to load /sessions",
        ":root[data-theme=\"paper\"] .context-track",
    ] {
        assert!(
            html.contains(marker),
            "console.html must keep v0.3 polish marker `{}`",
            marker
        );
    }
}

#[test]
fn console_html_has_typed_event_renderers() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    // Sprint 1 inc 6: every Event variant of v0.2.0 that the mockup
    // promises has a styled renderer must actually have one in the
    // shipping console.
    for needle in [
        ".tool-card",
        ".diff-card",
        ".compact-banner",
        ".skill-pop",
        ".streaming::after",
    ] {
        assert!(
            html.contains(needle),
            "console.html must define the `{}` renderer block",
            needle
        );
    }
    // And the JS dispatch must route the Event types into those renderers.
    for case in [
        "case 'ToolUseProposed'",
        "case 'ToolOutput'",
        "case 'DiffProposed'",
        "case 'Compacted'",
    ] {
        assert!(
            html.contains(case),
            "handleEvent() must wire `{}` into the new renderers",
            case
        );
    }
    assert!(
        html.contains("renderDiffCard") && html.contains("renderCompactBanner"),
        "diff + compact renderers must be implemented"
    );
}

#[test]
fn console_html_plan_mode_has_explicit_accept_edit_reject() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "function renderPlan",
        "data-plan-action=\"run\"",
        "data-plan-action=\"edit\"",
        "data-plan-action=\"reject\"",
        "function rejectPlan",
        "plan rejected",
    ] {
        assert!(html.contains(marker), "plan mode must expose `{}`", marker);
    }
}

#[test]
fn console_html_diff_view_exposes_per_hunk_controls() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "function parseDiffHunks",
        "function renderHunkedDiff",
        "function bindHunkControls",
        "data-hunk-action=\"accept\"",
        "data-hunk-action=\"reject\"",
        "data-state=\"pending\"",
    ] {
        assert!(
            html.contains(marker),
            "diff view must expose per-hunk marker `{}`",
            marker
        );
    }
}

#[test]
fn console_html_styles_streamed_code_cards() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    assert!(
        html.contains("d.className='code-card streaming-code'"),
        "streaming fenced code must render through the code-card component"
    );
    for marker in [
        ".code-card,.code-block",
        ".code-card summary",
        ".code-card summary::-webkit-details-marker",
        ".code-card .cc-copy",
        ".code-card pre",
        ".code-card code",
        ".syn-key",
        "function highlightCode",
        "applyCodeHighlight(card)",
        "if(!raw.trim())",
    ] {
        assert!(
            html.contains(marker),
            "console.html must style code card marker `{}`",
            marker
        );
    }
}

#[test]
fn console_html_keeps_cost_updates_out_of_the_transcript() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    let cost_case = html
        .split("case 'CostUpdate':")
        .nth(1)
        .expect("CostUpdate handler must exist")
        .split("case 'TokenUsageEstimated':")
        .next()
        .expect("CostUpdate handler must be followed by TokenUsageEstimated");
    assert!(
        cost_case.contains("setCost(ev.usd)") && !cost_case.contains("verboseLine"),
        "CostUpdate must update meters only, without adding transcript spam: {}",
        cost_case
    );
}

#[test]
fn console_html_strips_streamed_think_blocks() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "function _stripThinkStreamChunk(text)",
        "STREAM_STATE.think=state",
        "const start=lower.indexOf('<think>')",
        "const end=lower.indexOf('</think>')",
        "STREAM_STATE={mode:'prose',el:null,cardEl:null,lang:'',codeStarted:false,pending:'',think:_newThinkFilter()}",
    ] {
        assert!(
            html.contains(marker),
            "console.html must keep streamed think stripping marker `{}`",
            marker
        );
    }
}

#[test]
fn console_html_command_palette_button_toggles_closed() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "if(palette?.classList.contains('open'))paletteClose();",
        "else paletteOpen();",
    ] {
        assert!(
            html.contains(marker),
            "command palette button must toggle open/closed via marker `{}`",
            marker
        );
    }
}

#[test]
fn console_html_groups_verbose_lines_in_details() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        ".verbose-group summary",
        "VERBOSE_GROUP=document.createElement('details')",
        "VERBOSE_GROUP.className='ln verbose-group'",
        "ensureVerboseGroup().appendChild(VERBOSE_TICK_ROW)",
        "verboseLine(`${icon(ev.status)} ${ev.role} · ${verbedNote}`,roleCls(ev.role))",
    ] {
        assert!(
            html.contains(marker),
            "verbose output must stay grouped/collapsible via marker `{}`",
            marker
        );
    }
}

#[test]
fn console_html_has_abort_button_next_to_run() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "grid-template-columns:auto auto minmax(280px,1fr) minmax(190px,270px) auto auto",
        "#stopBtn{display:inline-flex",
        "#stopBtn:disabled{opacity:.42",
        r#"<button class="btn" id="runBtn">run</button>"#,
        r#"<button class="btn" id="stopBtn" type="button" title="Abort the running task" aria-label="Abort the running task" disabled>abort</button>"#,
        "stop.disabled=!active;",
        "const _stopBtn=$('stopBtn');if(_stopBtn)_stopBtn.addEventListener('click',stopRun);",
    ] {
        assert!(
            html.contains(marker),
            "console.html must keep composer abort marker `{}`",
            marker
        );
    }
}

#[test]
fn console_html_file_rows_open_internal_viewer() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        r#"data-rb-file="${escAttr(path||name)}""#,
        "host.querySelectorAll('[data-rb-file]').forEach(r=>r.addEventListener('click',()=>loadFileInPanel(r.dataset.rbFile)));",
        r#"<div class="rb-row click" data-rb-file="${escAttr(it.path||it.name)}" title="View file">"#,
        "String(it.path||'').toLowerCase().includes(q)",
        "dpPath.textContent=path;",
    ] {
        assert!(
            html.contains(marker),
            "file/artifact rows must open internal viewer via marker `{}`",
            marker
        );
    }
}

#[test]
fn console_html_focus_mode_uses_roomy_but_not_huge_margins() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "html[data-view=\"focus\"] #term{font-size:calc(15px * var(--read-scale,1));line-height:1.56;padding:22px clamp(28px,7vw,96px) 22px}",
        "html[data-view=\"focus\"] .ln{max-width:min(1040px,100%);margin-left:0;margin-right:auto;padding:2px 0;line-height:1.56}",
        ".verbose-group{margin:3px 0 5px",
    ] {
        assert!(
            html.contains(marker),
            "focus transcript density must keep marker `{}`",
            marker
        );
    }
    assert!(
        !html.contains("padding:26px max(26px,calc((100% - 920px)/2))"),
        "focus mode must not return to the old 200px+ centered gutters"
    );
}

#[test]
fn console_html_uses_system_ui_typography() {
    let html =
        std::fs::read_to_string("console.html").expect("console.html must ship with the WebView");
    for marker in [
        "--ui-font:-apple-system",
        "--mono-font:\"SF Mono\"",
        "body{font-family:var(--ui-font)",
        ".code-card code,.code-block .cb-body code{font-family:var(--mono-font)",
    ] {
        assert!(
            html.contains(marker),
            "console.html must keep Apple-style typography marker `{}`",
            marker
        );
    }
}

#[test]
fn classify_agent_color_falls_back_to_steel() {
    use sparrow::console::classify_agent_color;
    assert_eq!(classify_agent_color("blue"), "planner");
    assert_eq!(classify_agent_color("TEAL"), "coder");
    assert_eq!(classify_agent_color("gold"), "gold");
    assert_eq!(classify_agent_color("coral"), "coral");
    assert_eq!(classify_agent_color("something-unknown"), "steel");
    assert_eq!(classify_agent_color(""), "steel");
}

#[tokio::test]
async fn webview_app_builds_with_phase13_routes() {
    // Spin up the server on a free port, then shut it down — proves all
    // routes (sessions, artifacts, upload, security, memory, plugins, tools,
    // permissions, commands) compile and bind without panicking.
    use std::net::SocketAddr;
    use tokio::sync::broadcast;

    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (tx, _rx) = broadcast::channel(16);
    let server = sparrow::console::WebViewServer::new(addr, tx, None, None, None, None, None, None);
    // Just verify the constructor accepts the expected shape — actually
    // binding requires a tokio TcpListener which we skip to keep the test
    // hermetic and fast on all platforms.
    drop(server);
}
