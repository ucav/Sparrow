use std::io;
use std::time::Instant;

use crate::event::Event;
use crate::tui::theme::Theme;
use crossterm::{
    event::{self, Event as TermEvent, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use tokio::sync::mpsc;

pub mod formatters;
pub mod renderer;
pub mod theme;

/// Make the host console accept the UTF-8 output the TUI emits.
///
/// On Windows the default console code page (CP1252 / OEM-850) silently
/// corrupts every multi-byte character we draw — `•` becomes `â¢`, `·`
/// becomes `Â·`, box-drawing chars become noise, and a few bytes get
/// dropped along the way ("binary" → "binana"). The fix is a single
/// `SetConsoleOutputCP(65001)` call on stdout's code page before we
/// enter the alternate screen.
///
/// On Unix the terminal already speaks UTF-8 — no-op.
fn ensure_utf8_console() {
    #[cfg(windows)]
    {
        // Minimal FFI shim — equivalent to `chcp 65001` but applied to
        // the process's *output* code page only, so it does not leak into
        // child processes or the shell prompt after we exit.
        unsafe extern "system" {
            fn SetConsoleOutputCP(wCodePageID: u32) -> i32;
            fn SetConsoleCP(wCodePageID: u32) -> i32;
        }
        const CP_UTF8: u32 = 65001;
        unsafe {
            let _ = SetConsoleOutputCP(CP_UTF8);
            let _ = SetConsoleCP(CP_UTF8);
        }
    }
}
pub mod ansi_bridge;

type CrosstermTerminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;

#[derive(Debug, Clone)]
struct LogLine {
    text: String,
    style: LogStyle,
    indent: u16,
    /// If set, this line is a child of collapsible group N (hidden when collapsed).
    group: Option<usize>,
    /// If set, this line IS the collapsible header for group N.
    header_for: Option<usize>,
}

/// A collapsible task group in the scroll log (a run, an agent phase, a tool call).
#[derive(Debug, Clone)]
struct TaskGroup {
    title: String,
    collapsed: bool,
    style: LogStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LogStyle {
    Normal,
    Dim,
    Brand,
    Agent,
    Planner,
    Verifier,
    Rem,
    Steel,
    Gold,
    Prompt,
    Cmd,
    Ok,
    Warn,
    Err,
    Accent,
}

impl LogStyle {
    fn color(&self, theme: &Theme) -> Color {
        match self {
            LogStyle::Normal => theme.fg,
            LogStyle::Dim => theme.dim,
            LogStyle::Brand => theme.brand,
            LogStyle::Agent => theme.agent,
            LogStyle::Planner => theme.planner,
            LogStyle::Verifier => theme.verifier,
            LogStyle::Rem => theme.rem,
            LogStyle::Steel => theme.steel,
            LogStyle::Gold => theme.gold,
            LogStyle::Prompt => theme.brand,
            LogStyle::Cmd => theme.fg,
            LogStyle::Ok => theme.add,
            LogStyle::Warn => theme.verifier,
            LogStyle::Err => theme.rem,
            LogStyle::Accent => theme.brand,
        }
    }
}

const SLASH_COMMANDS: &[&str] = &[
    "/help",
    "/plan",
    "/permissions",
    "/memory",
    "/compact",
    "/model",
    "/agents",
    "/sessions",
    "/export",
    "/run",
    "/chat",
    "/swarm",
    "/agent",
    "/skills",
    "/checkpoint",
    "/rewind",
    "/replay",
    "/auth",
    "/clear",
    "/collapse",
    "/expand",
    "/exit",
];

const HISTORY_MAX: usize = 100;

// ─── Swarm lanes ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LaneState {
    /// AgentStatus name (Idle/Thinking/Working/Done/Error/WaitingForApproval)
    status: String,
    /// last note text
    note: String,
    /// Brain id
    model: String,
}

impl Default for LaneState {
    fn default() -> Self {
        Self {
            status: "Idle".into(),
            note: "".into(),
            model: "".into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct SwarmLanesState {
    planner: LaneState,
    coder: LaneState,
    verifier: LaneState,
    /// Frame at which the swarm started; used to fade-in lanes.
    started_at_frame: u64,
}

// ─── Diff panel ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum DiffLineKind {
    Context,
    Plus,
    Minus,
    Hunk,
}

#[derive(Debug, Clone)]
struct DiffLineEntry {
    kind: DiffLineKind,
    text: String,
}

#[derive(Debug, Clone)]
struct DiffEntry {
    file: String,
    plus: u32,
    minus: u32,
    lines: Vec<DiffLineEntry>,
    applied: bool,
}

fn parse_diff_patch(patch: &str) -> Vec<DiffLineEntry> {
    let mut out = Vec::new();
    for line in patch.lines().take(40) {
        let kind = if line.starts_with("+++") || line.starts_with("---") {
            DiffLineKind::Context
        } else if line.starts_with("@@") {
            DiffLineKind::Hunk
        } else if line.starts_with('+') {
            DiffLineKind::Plus
        } else if line.starts_with('-') {
            DiffLineKind::Minus
        } else {
            DiffLineKind::Context
        };
        out.push(DiffLineEntry {
            kind,
            text: line.to_string(),
        });
    }
    out
}

fn truncate_for_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    for ch in text.chars().take(width) {
        out.push(ch);
    }
    if text.chars().count() > width && width > 1 {
        out.pop();
        out.push('…');
    }
    out
}

fn syntax_spans(text: &str, theme: &Theme, base: Color) -> Vec<Span<'static>> {
    const KEYWORDS: &[&str] = &[
        "fn", "pub", "if", "else", "return", "let", "mut", "const", "struct", "impl", "trait",
        "use", "as", "match",
    ];
    let violet = Color::Rgb(0xb4, 0x8e, 0xff);
    let mut spans = Vec::new();
    let mut buf = String::new();
    let chars = text.chars();
    let mut in_string = false;

    let flush_word = |word: &mut String, spans: &mut Vec<Span<'static>>, next_is_call: bool| {
        if word.is_empty() {
            return;
        }
        let style = if KEYWORDS.contains(&word.as_str()) {
            Style::default().fg(violet).add_modifier(Modifier::BOLD)
        } else if next_is_call {
            Style::default().fg(theme.gold)
        } else {
            Style::default().fg(base)
        };
        spans.push(Span::styled(std::mem::take(word), style));
    };

    for ch in chars {
        if ch == '"' {
            if in_string {
                buf.push(ch);
                spans.push(Span::styled(
                    std::mem::take(&mut buf),
                    Style::default().fg(theme.add),
                ));
                in_string = false;
            } else {
                flush_word(&mut buf, &mut spans, false);
                buf.push(ch);
                in_string = true;
            }
            continue;
        }
        if in_string {
            buf.push(ch);
            continue;
        }
        if ch.is_alphanumeric() || ch == '_' {
            buf.push(ch);
            continue;
        }
        let next_is_call = ch == '(';
        flush_word(&mut buf, &mut spans, next_is_call);
        spans.push(Span::styled(ch.to_string(), Style::default().fg(base)));
    }
    if in_string {
        spans.push(Span::styled(buf, Style::default().fg(theme.add)));
    } else {
        flush_word(&mut buf, &mut spans, false);
    }
    spans
}

// ─── Checkpoint timeline ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct CheckpointNode {
    id: String,
    label: String,
    current: bool,
}

// ─── Embers (background particles) ───────────────────────────────────────────

#[derive(Debug, Clone)]
struct Ember {
    x: u16,
    y: f32,
    vy: f32,
    /// true = amber, false = coral
    amber: bool,
    life: u32,
    max_life: u32,
    /// char from the bird theme
    glyph: char,
}

// ─── Toast (skill learned, etc.) ─────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Toast {
    text: String,
    /// frames since spawn
    age: u32,
    /// total lifetime in frames
    max_age: u32,
}

pub struct Tui {
    theme: Theme,
    lines: Vec<LogLine>,
    route: String,
    cost_usd: f64,
    total_tokens: u64,
    autonomy: String,
    /// Multi-line input. input_lines[0] = first row of the prompt.
    input_lines: Vec<String>,
    /// Cursor row within input_lines.
    cursor_row: usize,
    /// Cursor col (byte index) within input_lines[cursor_row].
    cursor_col: usize,
    /// Command history, oldest first.
    history: Vec<String>,
    /// When navigating history, index into history; None = fresh editing.
    history_idx: Option<usize>,
    /// Pending injection mode: next Enter sends as injection, not new task.
    inject_pending: bool,
    scroll: u16,
    frame: u64,
    spinner_idx: usize,
    booted: bool,
    boot_progress: u32,
    event_rx: Option<mpsc::UnboundedReceiver<Event>>,
    task_tx: Option<mpsc::UnboundedSender<String>>,
    history_path: Option<std::path::PathBuf>,

    // ── Batch 3 additions ─────────────────────────────────────────────────
    /// Active swarm lanes (None when not in swarm mode).
    swarm_lanes: Option<SwarmLanesState>,
    /// Pending diffs (cap = 3, FIFO).
    pending_diffs: std::collections::VecDeque<DiffEntry>,
    /// Checkpoint timeline nodes.
    checkpoints: Vec<CheckpointNode>,
    /// Drifting embers in the scroll area.
    embers: Vec<Ember>,
    /// Centered overlay toast (skill learned, etc.).
    toast: Option<Toast>,
    /// Cost flash counter (frames remaining of bold cost).
    cost_flash_frames: u32,
    last_cost: f64,
    /// Token flash counter.
    tok_flash_frames: u32,
    last_tokens: u64,

    // ── Collapsible task groups ───────────────────────────────────────────
    /// Collapsible task groups; child lines reference these by index.
    groups: Vec<TaskGroup>,
    /// Group that new lines are attached to (None = top level).
    current_group: Option<usize>,
    /// Group header currently focused for collapse/expand (Ctrl+↑/↓, Ctrl+O).
    focus_group: Option<usize>,

    // ── Replay scrubber ───────────────────────────────────────────────────
    /// When set, the TUI is in replay mode: scrub events with ←/→.
    replay_events: Option<Vec<Event>>,
    replay_idx: usize,
    /// Strips <think> reasoning blocks from streamed deltas.
    think: crate::event::ThinkStripper,
    /// Known agent names for `@<name>` autocomplete; populated by the host.
    agent_names: Vec<String>,
    /// Currently active agent (toggled via @picker). None = default pipeline.
    active_agent: Option<String>,
    /// Cached agent souls: name → (role, personality_b64).
    agent_souls: std::collections::HashMap<String, (String, String)>,
    /// Rich terminal renderer (syntax highlighting, markdown, diffs).
    term_renderer: crate::tui::renderer::TermRenderer,
}

impl Tui {
    pub fn new() -> Self {
        // Resolve history path: ~/.local/state/sparrow/tui_history.txt
        let history_path = dirs::state_dir()
            .or_else(dirs::data_local_dir)
            .or_else(dirs::data_dir)
            .map(|d| d.join("sparrow").join("tui_history.txt"));
        let history = history_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| s.lines().map(String::from).collect())
            .unwrap_or_default();

        // Pick theme from $SPARROW_THEME or default to `captain`.
        let theme = std::env::var("SPARROW_THEME")
            .ok()
            .map(|n| crate::tui::theme::by_name(&n))
            .unwrap_or_default();
        let mut tui = Self {
            theme,
            lines: Vec::new(),
            route: "idle".into(),
            cost_usd: 0.0,
            total_tokens: 0,
            autonomy: "supervised".into(),
            input_lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            history,
            history_idx: None,
            inject_pending: false,
            scroll: 0,
            frame: 0,
            spinner_idx: 0,
            booted: false,
            boot_progress: 0,
            event_rx: None,
            task_tx: None,
            history_path,
            swarm_lanes: None,
            pending_diffs: std::collections::VecDeque::new(),
            checkpoints: Vec::new(),
            embers: Self::spawn_embers(),
            toast: None,
            cost_flash_frames: 0,
            last_cost: 0.0,
            tok_flash_frames: 0,
            last_tokens: 0,
            groups: Vec::new(),
            current_group: None,
            focus_group: None,
            replay_events: None,
            replay_idx: 0,
            think: crate::event::ThinkStripper::new(),
            agent_names: Vec::new(),
            active_agent: None,
            agent_souls: std::collections::HashMap::new(),
            term_renderer: crate::tui::renderer::TermRenderer::new(
                crate::tui::renderer::RenderConfig::default(),
            ),
        };
        tui.show_splash();
        tui
    }

    /// Show a rich-formatted splash screen demonstrating TUI capabilities.
    fn show_splash(&mut self) {
        self.add_line("══════════════════════════════════════", LogStyle::Brand, 0);
        self.add_line(
            "  🐦 SPARROW — one cli · grows with you",
            LogStyle::Brand,
            0,
        );
        self.add_line("══════════════════════════════════════", LogStyle::Brand, 0);
        self.add_line("", LogStyle::Cmd, 0);
        self.add_line("Try these (type in the input below):", LogStyle::Cmd, 0);
        self.add_line("  @nova     → Tab to toggle Nova agent", LogStyle::Dim, 0);
        self.add_line("  /help     → list all slash commands", LogStyle::Dim, 0);
        self.add_line("  Ctrl+R    → rewind to last checkpoint", LogStyle::Dim, 0);
        self.add_line("", LogStyle::Cmd, 0);
        // Demo: formatted code
        self.add_line("# RICH RENDERING DEMO", LogStyle::Gold, 0);
        self.add_line("", LogStyle::Cmd, 0);
        self.add_line("Code blocks get syntax highlighting:", LogStyle::Cmd, 0);
        self.add_line("```rust", LogStyle::Dim, 0);
        self.add_line("fn main() {", LogStyle::Cmd, 0);
        self.add_line("    println!(\"Hello, Sparrow!\");", LogStyle::Cmd, 0);
        self.add_line("}", LogStyle::Cmd, 0);
        self.add_line("```", LogStyle::Dim, 0);
        self.add_line("", LogStyle::Cmd, 0);
        self.add_line(
            "Diffs are colored (additions in green, deletions in red):",
            LogStyle::Cmd,
            0,
        );
        self.add_line("--- a/src/main.rs", LogStyle::Dim, 0);
        self.add_line("+++ b/src/main.rs", LogStyle::Dim, 0);
        self.add_line("@@ -10,6 +10,8 @@ fn main() {", LogStyle::Dim, 0);
        self.add_line("+    let config = load_config()?;", LogStyle::Ok, 0);
        self.add_line("     let engine = Engine::new();", LogStyle::Cmd, 0);
        self.add_line("-    engine.run_old();", LogStyle::Err, 0);
        self.add_line("+    engine.run_with_config(&config);", LogStyle::Ok, 0);
        self.add_line("", LogStyle::Cmd, 0);
        self.add_line("JSON is pretty-printed:", LogStyle::Cmd, 0);
        self.add_line("{", LogStyle::Dim, 0);
        self.add_line("  \"status\": \"ready\",", LogStyle::Ok, 0);
        self.add_line("  \"version\": \"0.5.9\",", LogStyle::Gold, 0);
        self.add_line(
            "  \"agents\": [\"nova\", \"planner\", \"coder\"]",
            LogStyle::Cmd,
            0,
        );
        self.add_line("}", LogStyle::Dim, 0);
        self.add_line("", LogStyle::Cmd, 0);
        self.add_line("→ Type a task or /command to begin.", LogStyle::Brand, 0);
    }

    /// Launch the TUI as a replay scrubber over a recorded transcript.
    /// ←/→ step through events; Home/End jump to start/end.
    pub fn with_replay(mut self, events: Vec<Event>) -> Self {
        self.replay_events = Some(events);
        self.replay_idx = 0;
        self.booted = true; // skip boot animation in replay mode
        self
    }

    /// Test-only: force the cockpit past the boot animation so a render
    /// snapshot exercises the live layout rather than the splash screen.
    #[doc(hidden)]
    pub fn force_booted(&mut self) {
        self.booted = true;
    }

    /// Test-only: drive the boot animation to a given progress so the splash
    /// renders its mid/late state (wordmark, boot log, ready) instead of the
    /// intentionally-blank first frame.
    #[doc(hidden)]
    pub fn debug_set_boot_progress(&mut self, progress: u32) {
        self.boot_progress = progress;
    }

    /// Test-only: render one frame to an in-memory [`TestBackend`] and return
    /// the buffer as plain text lines (one `String` per terminal row). This is
    /// the only way to exercise the real `render` path headlessly — the
    /// interactive `run`/`main_loop` require a live terminal.
    ///
    /// [`TestBackend`]: ratatui::backend::TestBackend
    #[doc(hidden)]
    pub fn render_to_lines(&mut self, width: u16, height: u16) -> Vec<String> {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|f| self.render(f, 0.0))
            .expect("render must not fail");
        let buf = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    /// Rebuild the log from replay events up to `replay_idx`.
    fn rebuild_replay(&mut self) {
        let Some(events) = self.replay_events.clone() else {
            return;
        };
        self.lines.clear();
        self.groups.clear();
        self.current_group = None;
        self.focus_group = None;
        self.cost_usd = 0.0;
        self.total_tokens = 0;
        let upto = self.replay_idx.min(events.len());
        for ev in events.iter().take(upto) {
            self.push_event(ev.clone());
        }
        let total = events.len();
        self.add_line(
            &format!(
                "── replay {}/{}  (←/→ step · Home/End jump · q quit) ──",
                upto, total
            ),
            LogStyle::Accent,
            0,
        );
    }

    fn spawn_embers() -> Vec<Ember> {
        // Deterministic-ish initial spread (no rand dep): use position + idx as seed.
        let glyphs = ['·', '•', '∘', '◦'];
        (0..10u16)
            .map(|i| Ember {
                x: 4 + (i * 13) % 90,
                y: 4.0 + ((i as f32) * 2.7) % 20.0,
                vy: 0.10 + ((i as f32) * 0.037) % 0.25,
                amber: i % 2 == 0,
                life: ((i as u32) * 17) % 180,
                max_life: 180 + ((i as u32) * 11) % 90,
                glyph: glyphs[(i as usize) % glyphs.len()],
            })
            .collect()
    }

    /// Snapshot current input as a single joined string.
    fn current_input(&self) -> String {
        self.input_lines.join("\n")
    }

    /// Replace current input with a single-line snapshot (used by history nav).
    fn set_input(&mut self, s: &str) {
        self.input_lines = s.split('\n').map(String::from).collect();
        if self.input_lines.is_empty() {
            self.input_lines.push(String::new());
        }
        self.cursor_row = self.input_lines.len() - 1;
        self.cursor_col = self.input_lines[self.cursor_row].len();
    }

    /// Append current input to history (de-dup against last entry) and persist.
    fn push_history(&mut self, entry: &str) {
        if entry.trim().is_empty() {
            return;
        }
        if self.history.last().map(|s| s.as_str()) == Some(entry) {
            return;
        }
        self.history.push(entry.to_string());
        if self.history.len() > HISTORY_MAX {
            let excess = self.history.len() - HISTORY_MAX;
            self.history.drain(..excess);
        }
        if let Some(path) = &self.history_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, self.history.join("\n"));
        }
    }

    /// Match autocomplete candidates for the current input.
    fn autocomplete_matches(&self) -> Vec<&'static str> {
        let line = &self.input_lines[0];
        if line.starts_with('/') {
            return SLASH_COMMANDS
                .iter()
                .filter(|c| c.starts_with(line.as_str()) && **c != line.as_str())
                .copied()
                .take(5)
                .collect();
        }
        vec![]
    }

    /// Test hook: mutable access to the first input line.
    #[doc(hidden)]
    pub fn debug_first_line_mut(&mut self) -> &mut String {
        if self.input_lines.is_empty() {
            self.input_lines.push(String::new());
        }
        &mut self.input_lines[0]
    }

    /// Test hook: set the cursor column.
    #[doc(hidden)]
    pub fn debug_set_cursor_col(&mut self, col: usize) {
        self.cursor_row = 0;
        self.cursor_col = col;
    }

    /// `@<name>` agent picker: returns owned strings prefixed with `@`. Separate
    /// from the slash autocomplete because the candidate list is dynamic.
    pub fn agent_matches(&self) -> Vec<String> {
        // Find the last `@` token on the current line.
        let line = &self.input_lines[self.cursor_row];
        let upto = line.get(..self.cursor_col).unwrap_or(line);
        let Some(at_pos) = upto.rfind('@') else {
            return vec![];
        };
        // Don't trigger when `@` is preceded by a non-whitespace char (so e-mails
        // like foo@example don't fire the picker).
        if at_pos > 0
            && !upto[..at_pos]
                .chars()
                .last()
                .map(|c| c.is_whitespace())
                .unwrap_or(true)
        {
            return vec![];
        }
        let prefix = &upto[at_pos + 1..];
        // Bail if the fragment already contains whitespace — picker is over.
        if prefix.contains(char::is_whitespace) {
            return vec![];
        }
        self.agent_names
            .iter()
            .filter(|n| n.starts_with(prefix))
            .take(5)
            .map(|n| format!("@{}", n))
            .collect()
    }

    /// Populate the `@<name>` agent picker with the agents the host knows about.
    pub fn with_agents(mut self, names: Vec<String>) -> Self {
        self.agent_names = names;
        self
    }

    /// Toggle an agent on/off. When toggled on, all subsequent tasks run with
    /// that agent's identity. Toggle again (or toggle another agent) to switch.
    pub fn toggle_agent(&mut self, name: &str) {
        if self.active_agent.as_deref() == Some(name) {
            // Deselect
            self.active_agent = None;
        } else {
            // Select — cache the agent soul
            self.active_agent = Some(name.to_string());
            if !self.agent_souls.contains_key(name) {
                self.cache_agent_soul(name);
            }
        }
    }

    /// Load and cache an agent's soul (role + base64 personality).
    fn cache_agent_soul(&mut self, name: &str) {
        let path = dirs::config_dir()
            .unwrap_or_default()
            .join("sparrow")
            .join("agents")
            .join(format!("{}.soul.toml", name));
        if let Ok(content) = std::fs::read_to_string(&path) {
            let role = content
                .lines()
                .find(|l| l.starts_with("role"))
                .and_then(|l| l.split('=').nth(1))
                .map(|s| s.trim().trim_matches('"').to_string())
                .unwrap_or_default();
            let personality = content
                .lines()
                .find(|l| l.starts_with("personality"))
                .and_then(|l| l.split('=').nth(1))
                .map(|s| s.trim().trim_matches('"').to_string())
                .unwrap_or_default();
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            let b64 = STANDARD.encode(personality.as_bytes());
            self.agent_souls.insert(name.to_string(), (role, b64));
        }
    }

    /// Build the agent dispatch prefix for task sending.
    fn agent_prefix(&self) -> String {
        if let Some(ref name) = self.active_agent {
            if let Some((role, b64)) = self.agent_souls.get(name) {
                return format!("__agent:{}__{}__{}__ ", name, role, b64);
            }
        }
        String::new()
    }

    pub fn with_channels(
        mut self,
        task_tx: mpsc::UnboundedSender<String>,
        event_rx: mpsc::UnboundedReceiver<Event>,
    ) -> Self {
        self.task_tx = Some(task_tx);
        self.event_rx = Some(event_rx);
        self
    }

    /// Format a log line with auto-detected content type.
    /// Applies syntax highlighting to code, colors to diffs, etc.
    fn format_line(&self, text: &str) -> String {
        // Detect content type
        let trimmed = text.trim();

        // Code blocks (start with ``` or indented 4+ spaces)
        if trimmed.starts_with("```") || text.lines().all(|l| l.starts_with("    ") || l.is_empty())
        {
            return self.term_renderer.render_code(text, "");
        }

        // Diff output (starts with diff --git, @@, +++, ---)
        if trimmed.contains("diff --git")
            || trimmed.starts_with("@@")
            || trimmed.starts_with("--- a/")
            || trimmed.starts_with("+++ b/")
        {
            return self.term_renderer.render_diff(text);
        }

        // JSON (starts with { or [)
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                return self.term_renderer.render_json(text);
            }
        }

        // Markdown headers (# Title, ## Section)
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ") {
            return self.term_renderer.render_markdown(text);
        }

        // Default: plain text
        text.to_string()
    }

    pub fn push_event(&mut self, event: Event) {
        match &event {
            Event::RunStarted { task, .. } => {
                self.think = crate::event::ThinkStripper::new();
                self.open_group(&format!("started: {}", task), LogStyle::Brand);
            }
            Event::RouteSelected { chain, .. } => {
                self.route = chain.join(" → ");
                self.add_line(&format!("↳ route: {}", self.route), LogStyle::Dim, 1);
            }
            Event::ModelSwitched {
                from, to, reason, ..
            } => {
                self.route = to.clone();
                let clean = crate::event::friendly_model_switch_reason(reason);
                let label = if crate::event::is_local_model_unavailable(reason) {
                    format!(
                        "↳ modèle local indisponible → routage modèle cloud ({})",
                        to
                    )
                } else {
                    format!("↳ fallback: {} → {} ({})", from, to, clean)
                };
                self.add_line(&label, LogStyle::Warn, 1);
            }
            Event::ThinkingDelta { text, .. } => {
                let visible = self.think.feed(text);
                if !visible.is_empty() {
                    self.add_line(&visible, LogStyle::Cmd, 1);
                }
            }
            Event::ReasoningDelta { .. } => {}
            Event::ToolUseProposed { name, .. } => {
                self.open_group(&format!("tool · {}", name), LogStyle::Steel);
            }
            Event::ToolOutput { blocks, .. } => {
                for b in blocks {
                    if let crate::event::Block::Text(t) = b {
                        self.add_line(&format!("  {}", t), LogStyle::Dim, 2);
                    }
                }
            }
            Event::AgentSpawned { role, model, .. } => {
                let lanes = self.swarm_lanes.get_or_insert_with(|| SwarmLanesState {
                    started_at_frame: self.frame,
                    ..Default::default()
                });
                let lane = match role.as_str() {
                    "planner" => &mut lanes.planner,
                    "coder" => &mut lanes.coder,
                    "verifier" => &mut lanes.verifier,
                    _ => &mut lanes.coder,
                };
                lane.status = "Working".into();
                lane.note = "spawned".into();
                lane.model = model.clone();
                let s = match role.as_str() {
                    "planner" => LogStyle::Planner,
                    "coder" => LogStyle::Agent,
                    "verifier" => LogStyle::Verifier,
                    _ => LogStyle::Dim,
                };
                self.open_group(&format!("{} ({})", role, model), s);
            }
            Event::AgentStatus {
                role, note, status, ..
            } => {
                if let Some(lanes) = self.swarm_lanes.as_mut() {
                    let lane = match role.as_str() {
                        "planner" => &mut lanes.planner,
                        "coder" => &mut lanes.coder,
                        "verifier" => &mut lanes.verifier,
                        _ => &mut lanes.coder,
                    };
                    lane.status = format!("{:?}", status);
                    lane.note = note.clone();
                }
                let s = match role.as_str() {
                    "planner" => LogStyle::Planner,
                    "coder" => LogStyle::Agent,
                    "verifier" => LogStyle::Verifier,
                    _ => LogStyle::Dim,
                };
                let icon = match status {
                    crate::event::AgentStatus::Done => "✓",
                    crate::event::AgentStatus::Working => "●",
                    crate::event::AgentStatus::Thinking => "○",
                    crate::event::AgentStatus::Error => "✗",
                    _ => "◌",
                };
                self.add_line(&format!("{} {} — {}", icon, role, note), s, 1);
            }
            Event::CheckpointCreated { id, label, .. } => {
                for node in &mut self.checkpoints {
                    node.current = false;
                }
                self.checkpoints.push(CheckpointNode {
                    id: id.0.clone(),
                    label: label.clone(),
                    current: true,
                });
                self.add_line(&format!("● checkpoint: {}", label), LogStyle::Gold, 0)
            }
            Event::SkillLearned { name, .. } => {
                self.toast = Some(Toast {
                    text: format!("✦ skill learned · {}", name),
                    age: 0,
                    max_age: 90,
                });
                self.add_line(&format!("✦ skill learned · {}", name), LogStyle::Agent, 0)
            }
            Event::CostUpdate { usd, .. } => {
                if *usd > self.last_cost {
                    self.cost_flash_frames = 12;
                }
                self.last_cost = *usd;
                self.cost_usd = *usd;
            }
            Event::TokenUsage { input, output, .. } => {
                self.total_tokens += input + output;
                if self.total_tokens > self.last_tokens {
                    self.tok_flash_frames = 12;
                }
                self.last_tokens = self.total_tokens;
            }
            Event::TokenUsageEstimated { input, output, .. } => {
                self.total_tokens += input + output;
                if self.total_tokens > self.last_tokens {
                    self.tok_flash_frames = 12;
                }
                self.last_tokens = self.total_tokens;
            }
            Event::AutonomyChanged { level, .. } => {
                self.autonomy = format!("{:?}", level).to_lowercase()
            }
            Event::DiffProposed {
                file,
                patch,
                plus,
                minus,
                ..
            } => {
                if self.pending_diffs.len() >= 3 {
                    self.pending_diffs.pop_front();
                }
                self.pending_diffs.push_back(DiffEntry {
                    file: file.clone(),
                    plus: *plus,
                    minus: *minus,
                    lines: parse_diff_patch(patch),
                    applied: false,
                });
                self.add_line(
                    &format!("◇ {}  +{} / -{}  · proposed", file, plus, minus),
                    LogStyle::Dim,
                    0,
                )
            }
            Event::DiffApplied { file, .. } => {
                if let Some(entry) = self.pending_diffs.iter_mut().find(|d| d.file == *file) {
                    entry.applied = true;
                }
                while self.pending_diffs.front().is_some_and(|d| d.applied) {
                    self.pending_diffs.pop_front();
                }
            }
            Event::TestResult {
                passed,
                failed,
                detail,
                ..
            } => {
                if *failed > 0 {
                    self.add_line(
                        &format!("⚠ tests  {} passed · {} failed", passed, failed),
                        LogStyle::Warn,
                        1,
                    );
                    for line in detail.lines() {
                        self.add_line(&format!("  {}", line), LogStyle::Rem, 2);
                    }
                } else {
                    self.add_line(
                        &format!("✓ tests  {} passed · no regressions", passed),
                        LogStyle::Ok,
                        1,
                    );
                }
            }
            Event::RunFinished { outcome, .. } => {
                // Recover any text held by the think-stripper (unclosed <think>).
                let tail = self.think.flush();
                if !tail.trim().is_empty() {
                    self.add_line(&tail, LogStyle::Cmd, 1);
                }
                self.close_group();
                self.add_line(
                    &format!(
                        "✓ done  status: {}  cost: ${:.4}",
                        outcome.status, outcome.cost_usd
                    ),
                    LogStyle::Ok,
                    0,
                );
                // Cost comparison — Sparrow's moat
                if outcome.tokens.input > 0 || outcome.tokens.output > 0 {
                    let comparison =
                        crate::cost::format_comparison(outcome.cost_usd, &outcome.tokens);
                    for line in comparison.lines().skip(1) {
                        // skip the "── Cost ──" header, show data lines
                        if !line.is_empty() && !line.starts_with("──") {
                            let style = if line.contains("Sparrow") {
                                LogStyle::Ok
                            } else if line.contains("💡") {
                                LogStyle::Warn
                            } else {
                                LogStyle::Rem
                            };
                            self.add_line(line, style, 1);
                        }
                    }
                }
            }
            Event::Error { message, .. } => {
                if !crate::event::is_local_model_unavailable(message) {
                    self.add_line(message, LogStyle::Err, 0);
                }
            }
            _ => {}
        }
    }

    fn add_line(&mut self, text: &str, style: LogStyle, indent: u16) {
        let group = self.current_group;
        for line in text.lines() {
            self.lines.push(LogLine {
                text: line.to_string(),
                style,
                indent,
                group,
                header_for: None,
            });
        }
    }

    /// Open a new collapsible task group; subsequent `add_line` calls attach to it.
    fn open_group(&mut self, title: &str, style: LogStyle) {
        let id = self.groups.len();
        self.groups.push(TaskGroup {
            title: title.to_string(),
            collapsed: false,
            style,
        });
        self.lines.push(LogLine {
            text: title.to_string(),
            style,
            indent: 0,
            group: None,
            header_for: Some(id),
        });
        self.current_group = Some(id);
        self.focus_group = Some(id);
    }

    /// Close the active group (subsequent lines go top-level).
    fn close_group(&mut self) {
        self.current_group = None;
    }

    /// Number of child lines belonging to a group (for the "N hidden" hint).
    fn group_child_count(&self, id: usize) -> usize {
        self.lines.iter().filter(|l| l.group == Some(id)).count()
    }

    /// Move focus to the previous/next group header.
    fn focus_group_step(&mut self, forward: bool) {
        if self.groups.is_empty() {
            return;
        }
        let last = self.groups.len() - 1;
        self.focus_group = Some(match self.focus_group {
            None => last,
            Some(i) if forward => (i + 1).min(last),
            Some(i) => i.saturating_sub(1),
        });
    }

    /// Toggle collapse on the focused group, or all groups if none focused.
    fn toggle_group(&mut self) {
        match self.focus_group {
            Some(i) if i < self.groups.len() => {
                self.groups[i].collapsed = !self.groups[i].collapsed;
            }
            _ => {
                let any_open = self.groups.iter().any(|g| !g.collapsed);
                for g in &mut self.groups {
                    g.collapsed = any_open;
                }
            }
        }
    }

    fn boot(&mut self) {
        self.add_line(
            concat!(
                "SPARROW  v",
                env!("CARGO_PKG_VERSION"),
                " — one cli · grows with you"
            ),
            LogStyle::Dim,
            0,
        );
        self.add_line("", LogStyle::Normal, 0);

        // Honest, platform-aware sandbox status. seccomp/namespaces are Linux-only;
        // on other platforms we run with workspace path-boundary enforcement only.
        #[cfg(target_os = "linux")]
        let sandbox_line = "local-hardened · namespaces + path boundary";
        #[cfg(not(target_os = "linux"))]
        let sandbox_line = "path-boundary enforcement (namespaces are Linux-only)";

        let boot = [
            (
                "router  ",
                "model routing + fallback chain",
                LogStyle::Planner,
            ),
            (
                "surfaces",
                "cli · tui · webview · gateway",
                LogStyle::Planner,
            ),
            ("sandbox ", sandbox_line, LogStyle::Ok),
            (
                "skills  ",
                "library indexed · self-improving",
                LogStyle::Accent,
            ),
            (
                "memory  ",
                "sqlite · bounded docs · session search",
                LogStyle::Ok,
            ),
            (
                "autonomy",
                "dial: supervised → trusted → autonomous",
                LogStyle::Accent,
            ),
        ];
        for (k, v, s) in &boot {
            self.add_line(&format!("{}  {}", k, v), *s, 1);
        }
        self.add_line("✓ ready  one binary. no dependencies.", LogStyle::Ok, 0);
        self.add_line("", LogStyle::Normal, 0);
        self.booted = true;
    }

    pub fn run(&mut self) -> io::Result<()> {
        // Windows: force the console code page to UTF-8 (65001) BEFORE we
        // enter the alternate screen. Without this the default CP1252/OEM
        // mangles every multi-byte glyph the TUI emits (•, ·, ∘, →, box-
        // drawing) into "â", "Â·" garbage and visibly drops bytes inside
        // ASCII strings, producing "binana"/"versioo" output.
        ensure_utf8_console();
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let mut terminal = ratatui::Terminal::new(backend)?;
        // Wipe any residue from the parent shell so ratatui starts on a
        // clean buffer (otherwise stray dots from the previous prompt show
        // up over empty panel areas).
        terminal.clear()?;
        let result = self.main_loop(&mut terminal);
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;
        result
    }

    fn main_loop(&mut self, terminal: &mut CrosstermTerminal) -> io::Result<()> {
        let start = Instant::now();
        if self.replay_events.is_some() {
            self.rebuild_replay();
        }
        loop {
            self.drain_engine_events();
            self.frame += 1;
            self.spinner_idx = (self.spinner_idx + 1) % 10;
            self.tick_visuals();
            terminal.draw(|f| self.render(f, start.elapsed().as_secs_f64()))?;
            if event::poll(std::time::Duration::from_millis(50))? {
                if let TermEvent::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                    match key.code {
                        KeyCode::Esc => break,
                        KeyCode::Char('c') if ctrl => break,

                        // ── Replay scrubber (active only in replay mode) ─────
                        KeyCode::Char('q') if self.replay_events.is_some() => break,
                        KeyCode::Left if self.replay_events.is_some() => {
                            self.replay_idx = self.replay_idx.saturating_sub(1);
                            self.rebuild_replay();
                        }
                        KeyCode::Right if self.replay_events.is_some() => {
                            let max = self.replay_events.as_ref().map(|e| e.len()).unwrap_or(0);
                            self.replay_idx = (self.replay_idx + 1).min(max);
                            self.rebuild_replay();
                        }
                        KeyCode::Home if self.replay_events.is_some() => {
                            self.replay_idx = 0;
                            self.rebuild_replay();
                        }
                        KeyCode::End if self.replay_events.is_some() => {
                            self.replay_idx =
                                self.replay_events.as_ref().map(|e| e.len()).unwrap_or(0);
                            self.rebuild_replay();
                        }

                        // Ctrl+L → clear log buffer
                        KeyCode::Char('l') if ctrl => {
                            self.lines.clear();
                        }
                        // Ctrl+I → next Enter sends as mid-run injection
                        KeyCode::Char('i') if ctrl => {
                            self.inject_pending = true;
                            self.add_line(
                                "[inject] next message will be sent to the running agent",
                                LogStyle::Warn,
                                0,
                            );
                        }

                        // ── Collapsible task groups ──────────────────────────
                        // Ctrl+↑/↓ move focus between task headers; Ctrl+O toggles.
                        KeyCode::Up if ctrl => self.focus_group_step(false),
                        KeyCode::Down if ctrl => self.focus_group_step(true),
                        KeyCode::Char('o') if ctrl => self.toggle_group(),

                        // History navigation (only when on first row of input)
                        KeyCode::Up if self.cursor_row == 0 && !self.history.is_empty() => {
                            let new_idx = match self.history_idx {
                                None => self.history.len() - 1,
                                Some(0) => 0,
                                Some(i) => i - 1,
                            };
                            self.history_idx = Some(new_idx);
                            let entry = self.history[new_idx].clone();
                            self.set_input(&entry);
                        }
                        KeyCode::Down if self.cursor_row == self.input_lines.len() - 1 => {
                            match self.history_idx {
                                Some(i) if i + 1 < self.history.len() => {
                                    self.history_idx = Some(i + 1);
                                    let entry = self.history[i + 1].clone();
                                    self.set_input(&entry);
                                }
                                Some(_) => {
                                    self.history_idx = None;
                                    self.set_input("");
                                }
                                None => {}
                            }
                        }

                        // Scrollback nav with PgUp/PgDn/Home/End
                        KeyCode::PageUp => self.scroll = self.scroll.saturating_add(10),
                        KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(10),
                        KeyCode::Home => self.scroll = 0,
                        KeyCode::End => self.scroll = u16::MAX,

                        // Tab → autocomplete or toggle agent
                        KeyCode::Tab => {
                            let line = &self.input_lines[0];
                            // @agent → toggle, not insert
                            if let Some(rest) = line.strip_prefix('@') {
                                let name = &rest.trim().to_string();
                                if !name.is_empty() && self.agent_names.contains(name) {
                                    self.toggle_agent(name);
                                    self.input_lines = vec![String::new()];
                                    self.cursor_row = 0;
                                    self.cursor_col = 0;
                                }
                            } else {
                                let matches = self.autocomplete_matches();
                                if let Some(first) = matches.first() {
                                    self.input_lines = vec![first.to_string()];
                                    self.cursor_row = 0;
                                    self.cursor_col = first.len();
                                }
                            }
                        }

                        // Backspace: handle multiline correctly
                        KeyCode::Backspace => {
                            if self.cursor_col > 0 {
                                let line = &mut self.input_lines[self.cursor_row];
                                let new_col = line[..self.cursor_col]
                                    .char_indices()
                                    .last()
                                    .map(|(i, _)| i)
                                    .unwrap_or(0);
                                line.replace_range(new_col..self.cursor_col, "");
                                self.cursor_col = new_col;
                            } else if self.cursor_row > 0 {
                                // join with previous line
                                let curr = self.input_lines.remove(self.cursor_row);
                                self.cursor_row -= 1;
                                let prev = &mut self.input_lines[self.cursor_row];
                                self.cursor_col = prev.len();
                                prev.push_str(&curr);
                            }
                        }

                        // Shift+Enter or Alt+Enter → newline
                        KeyCode::Enter if shift || key.modifiers.contains(KeyModifiers::ALT) => {
                            let line = &mut self.input_lines[self.cursor_row];
                            let rest = line.split_off(self.cursor_col);
                            self.cursor_row += 1;
                            self.cursor_col = 0;
                            self.input_lines.insert(self.cursor_row, rest);
                        }

                        // Enter → submit
                        KeyCode::Enter => {
                            let task = self.current_input().trim().to_string();
                            if !task.is_empty() {
                                // Handle in-TUI commands
                                match task.as_str() {
                                    "/clear" => {
                                        self.lines.clear();
                                        self.groups.clear();
                                        self.current_group = None;
                                        self.focus_group = None;
                                    }
                                    "/collapse" => {
                                        for g in &mut self.groups {
                                            g.collapsed = true;
                                        }
                                    }
                                    "/expand" => {
                                        for g in &mut self.groups {
                                            g.collapsed = false;
                                        }
                                    }
                                    "/exit" | "/quit" => break,
                                    "/help" => {
                                        self.add_line("Commands:", LogStyle::Brand, 0);
                                        for c in SLASH_COMMANDS {
                                            self.add_line(c, LogStyle::Dim, 1);
                                        }
                                        self.add_line(
                                            "Ctrl+I inject · Ctrl+L clear · Ctrl+↑/↓ focus task · Ctrl+O fold/unfold · Shift+Enter newline · Up/Down history",
                                            LogStyle::Dim, 0,
                                        );
                                        self.add_line(
                                            "/collapse · /expand — fold/unfold all tasks",
                                            LogStyle::Dim,
                                            1,
                                        );
                                    }
                                    s if s.starts_with("/plan") => {
                                        let planned = s.trim_start_matches("/plan").trim();
                                        if planned.is_empty() {
                                            self.add_line("Usage: /plan <task>", LogStyle::Warn, 0);
                                        } else {
                                            let plan =
                                                crate::plan::build_read_only_plan(planned, &[]);
                                            self.add_line(
                                                "Read-only plan · no tools or edits executed",
                                                LogStyle::Planner,
                                                0,
                                            );
                                            self.add_line(&plan.summary, LogStyle::Dim, 1);
                                            for (idx, step) in plan.steps.iter().enumerate() {
                                                self.add_line(
                                                    &format!("{}. {}", idx + 1, step),
                                                    LogStyle::Cmd,
                                                    1,
                                                );
                                            }
                                            self.add_line(
                                                "Run the task explicitly when you accept the plan.",
                                                LogStyle::Warn,
                                                0,
                                            );
                                        }
                                    }
                                    _ => {
                                        // Send to engine
                                        let label = if self.inject_pending {
                                            "inject"
                                        } else {
                                            "sparrow"
                                        };
                                        self.add_line(
                                            &format!("{} › {}", label, task.replace('\n', " ↵ ")),
                                            LogStyle::Prompt,
                                            0,
                                        );
                                        self.push_history(&task);
                                        let to_send = if self.inject_pending {
                                            format!("__inject__:{}", task)
                                        } else {
                                            let prefix = self.agent_prefix();
                                            if prefix.is_empty() {
                                                task.clone()
                                            } else {
                                                format!("{}{}", prefix, task)
                                            }
                                        };
                                        self.inject_pending = false;
                                        if let Some(tx) = &self.task_tx {
                                            if tx.send(to_send).is_err() {
                                                self.add_line(
                                                    "runtime channel disconnected",
                                                    LogStyle::Err,
                                                    0,
                                                );
                                            }
                                        }
                                    }
                                }
                                self.set_input("");
                                self.history_idx = None;
                            }
                        }

                        // Regular character → insert at cursor
                        KeyCode::Char(c) => {
                            let line = &mut self.input_lines[self.cursor_row];
                            line.insert(self.cursor_col, c);
                            self.cursor_col += c.len_utf8();
                        }

                        // Cursor movement
                        KeyCode::Left => {
                            if self.scroll == 0
                                && self.cursor_col == 0
                                && self.checkpoints.len() > 1
                            {
                                let previous = self
                                    .checkpoints
                                    .iter()
                                    .rev()
                                    .skip(1)
                                    .find(|node| !node.id.is_empty())
                                    .map(|node| node.id.clone());
                                if let (Some(id), Some(tx)) = (previous, &self.task_tx) {
                                    let _ = tx.send(format!("__rewind__:{}", id));
                                    self.add_line(
                                        "rewind requested from checkpoint timeline",
                                        LogStyle::Gold,
                                        0,
                                    );
                                }
                            } else if self.cursor_col > 0 {
                                self.cursor_col = self.input_lines[self.cursor_row]
                                    [..self.cursor_col]
                                    .char_indices()
                                    .last()
                                    .map(|(i, _)| i)
                                    .unwrap_or(0);
                            } else if self.cursor_row > 0 {
                                self.cursor_row -= 1;
                                self.cursor_col = self.input_lines[self.cursor_row].len();
                            }
                        }
                        KeyCode::Right => {
                            let line = &self.input_lines[self.cursor_row];
                            if self.cursor_col < line.len() {
                                let next = line[self.cursor_col..]
                                    .chars()
                                    .next()
                                    .map(|c| c.len_utf8())
                                    .unwrap_or(0);
                                self.cursor_col += next;
                            } else if self.cursor_row + 1 < self.input_lines.len() {
                                self.cursor_row += 1;
                                self.cursor_col = 0;
                            }
                        }

                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn tick_visuals(&mut self) {
        if !self.booted {
            self.boot_progress = self.boot_progress.saturating_add(1);
            if self.boot_progress >= 70 {
                self.boot();
            }
        }
        if self.cost_flash_frames > 0 {
            self.cost_flash_frames -= 1;
        }
        if self.tok_flash_frames > 0 {
            self.tok_flash_frames -= 1;
        }
        if let Some(toast) = self.toast.as_mut() {
            toast.age = toast.age.saturating_add(1);
            if toast.age >= toast.max_age {
                self.toast = None;
            }
        }
        for ember in &mut self.embers {
            ember.y -= ember.vy;
            ember.life = ember.life.saturating_add(1);
            if ember.life >= ember.max_life || ember.y < 0.0 {
                ember.y = 28.0 + (ember.x % 7) as f32;
                ember.life = 0;
            }
        }
    }

    fn drain_engine_events(&mut self) {
        let mut disconnected = false;
        let mut events = Vec::new();
        if let Some(rx) = self.event_rx.as_mut() {
            loop {
                match rx.try_recv() {
                    Ok(event) => events.push(event),
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
        }
        for event in events {
            self.push_event(event);
        }
        if disconnected {
            self.event_rx = None;
            self.add_line("runtime event stream disconnected", LogStyle::Warn, 0);
        }
    }

    fn render(&self, f: &mut Frame, _elapsed: f64) {
        let area = f.area();
        if !self.booted {
            self.render_boot(f, area);
            return;
        }
        // Input height = lines + 2 (border) + 1 (autocomplete row if any)
        let suggestions = self.autocomplete_matches();
        let input_height = (self.input_lines.len() as u16 + 2).max(3)
            + if !suggestions.is_empty() { 1 } else { 0 };
        let swarm_height = if self.swarm_lanes.is_some() { 5 } else { 0 };
        let diff_height = if self.pending_diffs.is_empty() { 0 } else { 12 };
        let checkpoint_height = if self.checkpoints.is_empty() { 0 } else { 2 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(swarm_height),
                Constraint::Min(0),
                Constraint::Length(diff_height),
                Constraint::Length(checkpoint_height),
                Constraint::Length(input_height),
            ])
            .split(area);
        self.render_cockpit(f, chunks[0]);
        if swarm_height > 0 {
            self.render_swarm_lanes(f, chunks[1]);
        }
        self.render_scroll(f, chunks[2]);
        if diff_height > 0 {
            self.render_diff(f, chunks[3]);
        }
        if checkpoint_height > 0 {
            self.render_checkpoint_timeline(f, chunks[4]);
        }
        self.render_input(f, chunks[5]);
        self.render_toast(f, area);
    }

    fn render_boot(&self, f: &mut Frame, area: Rect) {
        let mut lines = Vec::new();
        let bird_lines: Vec<&str> = theme::ASCII_SPARROW.lines().collect();
        let bird_count = ((self.boot_progress / 5) as usize).min(bird_lines.len());
        for line in bird_lines.iter().take(bird_count) {
            lines.push(Line::from(Span::styled(
                *line,
                Style::default().fg(self.theme.brand),
            )));
        }
        if self.boot_progress >= 25 {
            let wordmark = if self.boot_progress < 35 {
                "S  P  A  R  R  O  W"
            } else if self.boot_progress < 45 {
                "S P A R R O W"
            } else {
                "SPARROW"
            };
            lines.push(Line::from(Span::styled(
                wordmark,
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        #[cfg(target_os = "linux")]
        let sandbox_boot = "sandbox    local-hardened · namespaces armed";
        #[cfg(not(target_os = "linux"))]
        let sandbox_boot = "sandbox    path-boundary enforcement";
        let boot_log = [
            "router     warming provider graph",
            "surfaces   cli · webview · gateway",
            sandbox_boot,
            "skills     library indexed",
            "memory     sqlite profile loaded",
            "autonomy   dial ready",
        ];
        if self.boot_progress >= 45 {
            let count = (((self.boot_progress - 45) / 4) as usize).min(boot_log.len());
            for item in boot_log.iter().take(count) {
                lines.push(Line::from(Span::styled(
                    *item,
                    Style::default().fg(self.theme.dim),
                )));
            }
        }
        if self.boot_progress >= 68 {
            lines.push(Line::from(Span::styled(
                "✓ ready",
                Style::default()
                    .fg(self.theme.add)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        let height = lines.len() as u16;
        let width = area.width.min(72);
        let rect = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(height.max(1)) / 2,
            width,
            height: height.max(1),
        };
        f.render_widget(Paragraph::new(Text::from(lines)), rect);
    }

    fn render_cockpit(&self, f: &mut Frame, area: Rect) {
        let aut_color = match self.autonomy.as_str() {
            "autonomous" => self.theme.autonomous,
            "trusted" => self.theme.trusted,
            _ => self.theme.supervised,
        };

        // Spinner frame + flight verb cycling every ~25 frames (~1.25 s at 50 ms)
        let spinner = self.theme.spinner_frame(self.spinner_idx);
        let verb = self.theme.flight_verb(self.frame as usize / 25);

        // LED for autonomy pill: pulse between ● and ◉ every 8 frames
        let led = if self.frame / 8 % 2 == 0 {
            "●"
        } else {
            "◉"
        };

        // ── Right HUD zone (cost · tokens · autonomy pill) ────────────────
        // This block carries the load-bearing numbers — spend, token burn and
        // the autonomy level. It is laid out in its own right-aligned chunk so
        // it stays visible even on an 80-column terminal; only the route (left
        // zone) truncates when space is tight, never the budget readout.
        let cost_str = if self.cost_usd > 0.0 {
            format!("${:.4} ▲  ", self.cost_usd)
        } else {
            format!("${:.4}  ", self.cost_usd)
        };
        let tok_str = format!("{} tok  ", self.total_tokens);
        let aut_upper = self.autonomy.to_uppercase();
        // Reserve the plain-text width of the right zone (+1 leading gap).
        let right_w = (cost_str.chars().count()
            + tok_str.chars().count()
            + 2 // led + space
            + aut_upper.chars().count()
            + 1) as u16;

        let right = Line::from(vec![
            Span::styled(
                cost_str,
                if self.cost_flash_frames > 0 {
                    Style::default()
                        .fg(self.theme.gold)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.theme.brand)
                },
            ),
            // tokens
            Span::styled(
                tok_str,
                if self.tok_flash_frames > 0 {
                    Style::default()
                        .fg(self.theme.gold)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(self.theme.steel)
                },
            ),
            // autonomy pill with pulsing LED
            Span::styled(
                format!("{} ", led),
                Style::default().fg(aut_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                aut_upper,
                Style::default().fg(aut_color).add_modifier(Modifier::BOLD),
            ),
        ]);

        // Draw the frame once, then split its interior so the right HUD gets a
        // reserved, right-aligned column and the left zone takes the rest.
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.line));
        let inner = block.inner(area);
        f.render_widget(block, area);
        let zones = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(right_w)])
            .split(inner);

        // ── Left zone (spinner · wordmark · verb · agent · route) ─────────
        // The route is the flexible element: rather than let the paragraph clip
        // it mid-word, pre-truncate it with an ellipsis to the space the left
        // zone actually has, so narrow terminals read "…coder" not "qwen2.5-cod".
        let agent_badge = match &self.active_agent {
            // 🐦 renders two cells wide; count it as 2 for the width budget.
            Some(agent) => format!("🐦 {}  ", agent.to_uppercase()),
            None => String::new(),
        };
        let agent_w = if agent_badge.is_empty() {
            0
        } else {
            agent_badge.chars().count() + 1 // +1 for the wide bird glyph
        };
        // Fixed left prefix: spinner(2) + "SPARROW  "(9) + verb field(11) +
        // agent badge + "route: "(7).
        let prefix_w = 2 + 9 + 11 + agent_w + 7;
        let route_budget = (zones[0].width as usize).saturating_sub(prefix_w);
        let route_disp = truncate_for_width(&self.route, route_budget);
        let left = Line::from(vec![
            Span::styled(
                format!("{} ", spinner),
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "SPARROW  ",
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<9}  ", verb),
                Style::default().fg(self.theme.dim),
            ),
            Span::styled(
                agent_badge,
                Style::default()
                    .fg(self.theme.gold)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("route: {}", route_disp),
                Style::default().fg(self.theme.planner),
            ),
        ]);
        f.render_widget(Paragraph::new(left), zones[0]);
        f.render_widget(
            Paragraph::new(right).alignment(ratatui::layout::Alignment::Right),
            zones[1],
        );
    }

    fn render_swarm_lanes(&self, f: &mut Frame, area: Rect) {
        let Some(lanes) = &self.swarm_lanes else {
            return;
        };
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(area);
        let age = self.frame.saturating_sub(lanes.started_at_frame);
        let items = [
            ("planner", &lanes.planner, self.theme.planner),
            ("coder", &lanes.coder, self.theme.agent),
            ("verifier", &lanes.verifier, self.theme.verifier),
        ];
        for (idx, (role, lane, color)) in items.iter().enumerate() {
            let working = lane.status == "Working" || lane.status == "Thinking";
            let icon = match lane.status.as_str() {
                "Done" => "✓",
                "Error" => "✗",
                "Idle" => "◌",
                _ if self.frame / 8 % 2 == 0 => "●",
                _ => "○",
            };
            let caret = if working && self.frame / 8 % 2 == 0 {
                " ▌"
            } else {
                ""
            };
            let note_width = cols[idx].width.saturating_sub(4) as usize;
            let note = truncate_for_width(&lane.note, note_width);
            let lines = vec![
                Line::from(Span::styled(
                    format!("{}  {}", role.to_uppercase(), lane.model),
                    Style::default().fg(*color).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    format!("{}  {}{}", icon, lane.status, caret),
                    Style::default().fg(if working { self.theme.gold } else { *color }),
                )),
                Line::from(Span::styled(note, Style::default().fg(self.theme.fg))),
            ];
            f.render_widget(
                Paragraph::new(Text::from(lines)).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("swarm {}", age.min(99)))
                        .border_style(Style::default().fg(*color)),
                ),
                cols[idx],
            );
        }
    }

    fn render_scroll(&self, f: &mut Frame, area: Rect) {
        let max_lines = area.height.saturating_sub(2) as usize;
        if max_lines == 0 {
            return;
        }
        // Filter out child lines of collapsed groups; render headers as toggles.
        let rendered: Vec<Line> = self
            .lines
            .iter()
            .filter_map(|log| {
                // Hide children of collapsed groups
                if let Some(g) = log.group {
                    if self.groups.get(g).map(|gr| gr.collapsed).unwrap_or(false) {
                        return None;
                    }
                }
                if let Some(gid) = log.header_for {
                    // Collapsible header: ▾ expanded / ▸ collapsed + child count + focus mark
                    let gr = self.groups.get(gid);
                    let collapsed = gr.map(|g| g.collapsed).unwrap_or(false);
                    let title = gr.map(|g| g.title.as_str()).unwrap_or(log.text.as_str());
                    let log_style = gr.map(|g| g.style).unwrap_or(log.style);
                    let arrow = if collapsed { "▸" } else { "▾" };
                    let focused = self.focus_group == Some(gid);
                    let n = self.group_child_count(gid);
                    let hint = if collapsed && n > 0 {
                        format!("  ({} hidden)", n)
                    } else {
                        String::new()
                    };
                    let marker = if focused { "‣ " } else { "  " };
                    let mut style = Style::default().fg(log_style.color(&self.theme));
                    if focused {
                        style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
                    }
                    Some(Line::from(Span::styled(
                        format!("{}{} {}{}", marker, arrow, title, hint),
                        style,
                    )))
                } else {
                    let formatted = self.format_line(&log.text);
                    let prefix = "  ".repeat(log.indent as usize);
                    let rendered_line = crate::tui::ansi_bridge::render_line(
                        &formatted,
                        Style::default().fg(log.style.color(&self.theme)),
                    );
                    // Prepend indent prefix
                    let mut final_spans =
                        vec![Span::styled(prefix, Style::default().fg(self.theme.dim))];
                    final_spans.extend(rendered_line.spans);
                    Some(Line::from(final_spans))
                }
            })
            .collect();

        let total = rendered.len();
        let skip = (self.scroll as usize).min(total.saturating_sub(1));
        let show_logo = self.frame.saturating_sub(70) < 120 && self.scroll == 0;
        let logo_lines: Vec<Line> = if show_logo {
            theme::ascii_sparrow_at_frame(self.frame)
                .lines()
                .map(|line| {
                    Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(self.theme.brand),
                    ))
                })
                .collect()
        } else {
            Vec::new()
        };
        let remaining = max_lines.saturating_sub(logo_lines.len());
        let mut text_lines: Vec<Line> = logo_lines;
        let start = total.saturating_sub(skip).saturating_sub(remaining);
        let end = total.saturating_sub(skip);
        text_lines.extend(rendered[start..end].iter().cloned());
        f.render_widget(
            Paragraph::new(Text::from(text_lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.line)),
            ),
            area,
        );
        self.render_embers(f, area);
    }

    fn render_embers(&self, f: &mut Frame, area: Rect) {
        if area.width < 3 || area.height < 3 {
            return;
        }
        for ember in &self.embers {
            let x = area.x + 1 + (ember.x % area.width.saturating_sub(2));
            let y_offset = (ember.y.max(0.0) as u16) % area.height.saturating_sub(2);
            let y = area.y + 1 + y_offset;
            let color = if ember.amber {
                self.theme.gold
            } else {
                self.theme.rem
            };
            if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                cell.set_char(ember.glyph).set_fg(color);
            }
        }
    }

    fn render_diff(&self, f: &mut Frame, area: Rect) {
        let Some(diff) = self.pending_diffs.back() else {
            return;
        };
        let mut lines = vec![Line::from(vec![
            Span::styled("◇ ", Style::default().fg(self.theme.gold)),
            Span::styled(
                truncate_for_width(&diff.file, area.width.saturating_sub(20) as usize),
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  +{} / -{}  · proposed", diff.plus, diff.minus),
                Style::default().fg(self.theme.dim),
            ),
        ])];
        for (idx, line) in diff
            .lines
            .iter()
            .take(area.height.saturating_sub(3) as usize)
            .enumerate()
        {
            let color = match line.kind {
                DiffLineKind::Plus => self.theme.add,
                DiffLineKind::Minus => self.theme.rem,
                DiffLineKind::Hunk => self.theme.gold,
                DiffLineKind::Context => self.theme.dim,
            };
            let mut spans = vec![Span::styled(
                format!("{:>4} ", idx + 1),
                Style::default().fg(self.theme.dimmer),
            )];
            spans.extend(syntax_spans(&line.text, &self.theme, color));
            lines.push(Line::from(spans));
        }
        f.render_widget(
            Paragraph::new(Text::from(lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("diff")
                    .border_style(Style::default().fg(self.theme.line)),
            ),
            area,
        );
    }

    fn render_checkpoint_timeline(&self, f: &mut Frame, area: Rect) {
        let mut spans = Vec::new();
        for (idx, node) in self
            .checkpoints
            .iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .enumerate()
        {
            if idx > 0 {
                spans.push(Span::styled("──", Style::default().fg(self.theme.dimmer)));
            }
            spans.push(Span::styled(
                if node.current { "●" } else { "◆" },
                Style::default().fg(if node.current {
                    self.theme.gold
                } else {
                    self.theme.dim
                }),
            ));
        }
        if let Some(current) = self.checkpoints.iter().find(|n| n.current) {
            spans.push(Span::styled(
                format!(
                    "  {} · {}",
                    truncate_for_width(&current.label, 36),
                    current.id.chars().take(8).collect::<String>()
                ),
                Style::default().fg(self.theme.dim),
            ));
        }
        spans.push(Span::styled(
            "    rewind ← · snapshot before each batch",
            Style::default().fg(self.theme.dimmer),
        ));
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn render_toast(&self, f: &mut Frame, area: Rect) {
        let Some(toast) = &self.toast else {
            return;
        };
        let width = (toast.text.chars().count() as u16 + 6).min(area.width.saturating_sub(2));
        if width < 8 || area.height < 5 {
            return;
        }
        let rect = Rect {
            x: area.x + area.width.saturating_sub(width) / 2,
            y: area.y + area.height.saturating_sub(3) / 2,
            width,
            height: 3,
        };
        let border = if toast.age / 20 % 2 == 0 {
            Style::default()
                .fg(self.theme.gold)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.theme.gold)
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                toast.text.as_str(),
                Style::default()
                    .fg(self.theme.gold)
                    .add_modifier(Modifier::BOLD),
            )))
            .block(Block::default().borders(Borders::ALL).border_style(border)),
            rect,
        );
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let cursor_char = if self.frame / 8 % 2 == 0 { "▌" } else { " " };
        let prompt = if self.inject_pending {
            "◆ inject › "
        } else {
            "◆ sparrow › "
        };
        let prompt_color = if self.inject_pending {
            self.theme.coral
        } else {
            self.theme.brand
        };

        let mut text_lines: Vec<Line> = Vec::new();
        for (row_idx, line) in self.input_lines.iter().enumerate() {
            let mut spans: Vec<Span> = Vec::new();
            if row_idx == 0 {
                spans.push(Span::styled(
                    prompt,
                    Style::default()
                        .fg(prompt_color)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    "          › ",
                    Style::default().fg(self.theme.dimmer),
                ));
            }
            if row_idx == self.cursor_row {
                let (before, after) = line.split_at(self.cursor_col.min(line.len()));
                spans.push(Span::styled(before, Style::default().fg(self.theme.fg)));
                spans.push(Span::styled(cursor_char, Style::default().fg(prompt_color)));
                spans.push(Span::styled(after, Style::default().fg(self.theme.fg)));
            } else {
                spans.push(Span::styled(
                    line.as_str(),
                    Style::default().fg(self.theme.fg),
                ));
            }
            text_lines.push(Line::from(spans));
        }

        // Autocomplete row (suggestions)
        let suggestions = self.autocomplete_matches();
        if !suggestions.is_empty() {
            let mut s: Vec<Span> = vec![Span::styled(
                "  ⇥  ",
                Style::default().fg(self.theme.dimmer),
            )];
            for (i, cmd) in suggestions.iter().enumerate() {
                if i == 0 {
                    s.push(Span::styled(
                        *cmd,
                        Style::default()
                            .fg(self.theme.brand)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    s.push(Span::styled(*cmd, Style::default().fg(self.theme.dim)));
                }
                s.push(Span::raw("  "));
            }
            text_lines.push(Line::from(s));
        }

        f.render_widget(
            Paragraph::new(Text::from(text_lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.line)),
            ),
            area,
        );
    }
}

impl Default for Tui {
    fn default() -> Self {
        Self::new()
    }
}
