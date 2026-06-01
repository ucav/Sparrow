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

pub mod theme;

type CrosstermTerminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;

#[derive(Debug, Clone)]
struct LogLine {
    text: String,
    style: LogStyle,
    indent: u16,
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
    "/run",
    "/chat",
    "/swarm",
    "/agent",
    "/skills",
    "/checkpoint",
    "/rewind",
    "/replay",
    "/model",
    "/auth",
    "/help",
    "/clear",
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

        Self {
            theme: Theme::default(),
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
        }
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
        if !line.starts_with('/') {
            return vec![];
        }
        SLASH_COMMANDS
            .iter()
            .filter(|c| c.starts_with(line.as_str()) && **c != line.as_str())
            .copied()
            .take(5)
            .collect()
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

    pub fn push_event(&mut self, event: Event) {
        match &event {
            Event::RunStarted { task, .. } => {
                self.add_line(&format!("▸ started: {}", task), LogStyle::Brand, 0)
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
            Event::ThinkingDelta { text, .. } => self.add_line(text, LogStyle::Cmd, 1),
            Event::ToolUseProposed { name, .. } => {
                self.add_line(&format!("◆ {}", name), LogStyle::Steel, 1)
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
                self.add_line(&format!("◆ {} spawned ({})", role, model), s, 0);
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
            Event::RunFinished { outcome, .. } => self.add_line(
                &format!(
                    "✓ done  status: {}  cost: ${:.4}",
                    outcome.status, outcome.cost_usd
                ),
                LogStyle::Ok,
                0,
            ),
            Event::Error { message, .. } => {
                if !crate::event::is_local_model_unavailable(message) {
                    self.add_line(message, LogStyle::Err, 0);
                }
            }
            _ => {}
        }
    }

    fn add_line(&mut self, text: &str, style: LogStyle, indent: u16) {
        for line in text.lines() {
            self.lines.push(LogLine {
                text: line.to_string(),
                style,
                indent,
            });
        }
    }

    fn boot(&mut self) {
        self.add_line(
            "SPARROW  v0.1.0 — one cli · grows with you",
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
            ("memory  ", "4 tiers · sqlite profile", LogStyle::Ok),
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
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let mut terminal = ratatui::Terminal::new(backend)?;
        let result = self.main_loop(&mut terminal);
        disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;
        result
    }

    fn main_loop(&mut self, terminal: &mut CrosstermTerminal) -> io::Result<()> {
        let start = Instant::now();
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

                        // Tab → autocomplete first slash-command match
                        KeyCode::Tab => {
                            let matches = self.autocomplete_matches();
                            if let Some(first) = matches.first() {
                                self.input_lines = vec![first.to_string()];
                                self.cursor_row = 0;
                                self.cursor_col = first.len();
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
                                    }
                                    "/exit" | "/quit" => break,
                                    "/help" => {
                                        self.add_line("Commands:", LogStyle::Brand, 0);
                                        for c in SLASH_COMMANDS {
                                            self.add_line(c, LogStyle::Dim, 1);
                                        }
                                        self.add_line(
                                            "Ctrl+I = inject mid-run · Ctrl+L = clear · Shift+Enter = newline · Up/Down = history",
                                            LogStyle::Dim, 0,
                                        );
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
                                            task.clone()
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

        let line = Line::from(vec![
            // braille spinner
            Span::styled(
                format!("{} ", spinner),
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            // wordmark
            Span::styled(
                "SPARROW  ",
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            // flight verb (cycling, fixed-width so cockpit doesn't jump)
            Span::styled(
                format!("{:<9}  ", verb),
                Style::default().fg(self.theme.dim),
            ),
            // route
            Span::styled(
                format!("route: {}  ", self.route),
                Style::default().fg(self.theme.planner),
            ),
            // cost with ▲ when non-zero
            Span::styled(
                if self.cost_usd > 0.0 {
                    format!("${:.4} ▲  ", self.cost_usd)
                } else {
                    format!("${:.4}  ", self.cost_usd)
                },
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
                format!("{} tok  ", self.total_tokens),
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
                self.autonomy.to_uppercase(),
                Style::default().fg(aut_color).add_modifier(Modifier::BOLD),
            ),
        ]);
        f.render_widget(
            Paragraph::new(line).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.line)),
            ),
            area,
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
        let total = self.lines.len();
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
        let visible = self.lines.iter().rev().skip(skip).take(remaining);
        let mut text_lines: Vec<Line> = logo_lines;
        text_lines.extend(visible.rev().map(|log| {
            let prefix = "  ".repeat(log.indent as usize);
            Line::from(Span::styled(
                format!("{}{}", prefix, log.text),
                Style::default().fg(log.style.color(&self.theme)),
            ))
        }));
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
