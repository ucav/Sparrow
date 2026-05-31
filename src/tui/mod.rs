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

pub struct Tui {
    theme: Theme,
    lines: Vec<LogLine>,
    route: String,
    cost_usd: f64,
    total_tokens: u64,
    autonomy: String,
    input: String,
    scroll: u16,
    frame: u64,
    spinner_idx: usize,
    booted: bool,
    event_rx: Option<mpsc::UnboundedReceiver<Event>>,
    task_tx: Option<mpsc::UnboundedSender<String>>,
}

impl Tui {
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
            lines: Vec::new(),
            route: "idle".into(),
            cost_usd: 0.0,
            total_tokens: 0,
            autonomy: "supervised".into(),
            input: String::new(),
            scroll: 0,
            frame: 0,
            spinner_idx: 0,
            booted: false,
            event_rx: None,
            task_tx: None,
        }
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
            Event::CheckpointCreated { label, .. } => {
                self.add_line(&format!("● checkpoint: {}", label), LogStyle::Gold, 0)
            }
            Event::SkillLearned { name, .. } => {
                self.add_line(&format!("✦ skill learned · {}", name), LogStyle::Agent, 0)
            }
            Event::CostUpdate { usd, .. } => self.cost_usd = *usd,
            Event::TokenUsage { input, output, .. } => self.total_tokens += input + output,
            Event::TokenUsageEstimated { input, output, .. } => self.total_tokens += input + output,
            Event::AutonomyChanged { level, .. } => {
                self.autonomy = format!("{:?}", level).to_lowercase()
            }
            Event::DiffProposed {
                file, plus, minus, ..
            } => self.add_line(
                &format!("◇ {}  +{} / -{}  · proposed", file, plus, minus),
                LogStyle::Dim,
                0,
            ),
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
        for l in theme::ASCII_SPARROW.lines() {
            self.add_line(l, LogStyle::Brand, 0);
        }
        self.add_line(
            "SPARROW  v0.1.0 — one cli · grows with you",
            LogStyle::Dim,
            0,
        );
        self.add_line("", LogStyle::Normal, 0);
        let boot = [
            ("router  ", "5 providers online", LogStyle::Planner),
            (
                "surfaces",
                "cli · telegram · discord · slack",
                LogStyle::Planner,
            ),
            ("sandbox ", "seccomp + namespaces armed", LogStyle::Ok),
            ("skills  ", "47 loaded · self-improving", LogStyle::Accent),
            (
                "memory  ",
                "4 tiers · knows you across sessions",
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
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = ratatui::backend::CrosstermBackend::new(stdout);
        let mut terminal = ratatui::Terminal::new(backend)?;
        self.boot();
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
            terminal.draw(|f| self.render(f, start.elapsed().as_secs_f64()))?;
            if event::poll(std::time::Duration::from_millis(50))? {
                if let TermEvent::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break;
                        }
                        KeyCode::Up => self.scroll = self.scroll.saturating_add(1),
                        KeyCode::Down => self.scroll = self.scroll.saturating_sub(1),
                        KeyCode::PageUp => self.scroll = self.scroll.saturating_add(10),
                        KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(10),
                        KeyCode::Home => self.scroll = 0,
                        KeyCode::End => self.scroll = u16::MAX,
                        KeyCode::Char(c) => self.input.push(c),
                        KeyCode::Backspace => {
                            self.input.pop();
                        }
                        KeyCode::Enter => {
                            let task = self.input.trim().to_string();
                            if !task.is_empty() {
                                self.add_line(&format!("sparrow › {}", task), LogStyle::Prompt, 0);
                                if let Some(tx) = &self.task_tx {
                                    if tx.send(task).is_err() {
                                        self.add_line(
                                            "runtime channel disconnected",
                                            LogStyle::Err,
                                            0,
                                        );
                                    }
                                }
                                self.input.clear();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(area);
        self.render_cockpit(f, chunks[0]);
        self.render_scroll(f, chunks[1]);
        self.render_input(f, chunks[2]);
    }

    fn render_cockpit(&self, f: &mut Frame, area: Rect) {
        let aut_color = match self.autonomy.as_str() {
            "autonomous" => self.theme.autonomous,
            "trusted" => self.theme.trusted,
            _ => self.theme.supervised,
        };
        let spinner = self.theme.spinner_frame(self.spinner_idx);
        let line = Line::from(vec![
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
                format!("route: {}  ", self.route),
                Style::default().fg(self.theme.planner),
            ),
            Span::styled(
                format!("${:.4}  ", self.cost_usd),
                Style::default().fg(self.theme.brand),
            ),
            Span::styled(
                format!("{} tok  ", self.total_tokens),
                Style::default().fg(self.theme.steel),
            ),
            Span::styled(
                format!("⬡ {}", self.autonomy.to_uppercase()),
                Style::default().fg(aut_color),
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

    fn render_scroll(&self, f: &mut Frame, area: Rect) {
        let max_lines = area.height.saturating_sub(2) as usize;
        if max_lines == 0 {
            return;
        }
        let total = self.lines.len();
        let skip = (self.scroll as usize).min(total.saturating_sub(1));
        let visible = self.lines.iter().rev().skip(skip).take(max_lines);
        let text_lines: Vec<Line> = visible
            .rev()
            .map(|log| {
                let prefix = "  ".repeat(log.indent as usize);
                Line::from(Span::styled(
                    format!("{}{}", prefix, log.text),
                    Style::default().fg(log.style.color(&self.theme)),
                ))
            })
            .collect();
        f.render_widget(
            Paragraph::new(Text::from(text_lines)).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.theme.line)),
            ),
            area,
        );
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let cursor = if self.frame / 8 % 2 == 0 { "▌" } else { " " };
        let line = Line::from(vec![
            Span::styled(
                "◆ sparrow › ",
                Style::default()
                    .fg(self.theme.brand)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&self.input, Style::default().fg(self.theme.fg)),
            Span::styled(cursor, Style::default().fg(self.theme.brand)),
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
}

impl Default for Tui {
    fn default() -> Self {
        Self::new()
    }
}
