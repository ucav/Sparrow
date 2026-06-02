use ratatui::style::Color;

// ─── Color tokens from §9.2 ─────────────────────────────────────────────────────

pub struct Theme {
    pub brand: Color,      // #f2a93c amber — brand, cost
    pub coral: Color,      // #f0674a coral — secondary accent
    pub agent: Color,      // #4ec9b0 teal — active agent / coder
    pub planner: Color,    // #6fa6e6 blue — routing / planner
    pub verifier: Color,   // #c9a14e sand — verifier
    pub add: Color,        // #74c258 green — diff +
    pub rem: Color,        // #d96a63 red — diff -
    pub gold: Color,       // #f2c94c — pirate hoop / highlights
    pub steel: Color,      // #b9b0a3 — tool metal
    pub supervised: Color, // #74c258 green
    pub trusted: Color,    // #f2a93c amber
    pub autonomous: Color, // #d96a63 red
    pub bg: Color,         // #0e0b08 near-black
    pub panel: Color,      // #16120d panel bg
    pub line: Color,       // #2c251c hairline
    pub fg: Color,         // #ece2cf text
    pub dim: Color,        // #897d6c muted
    pub dimmer: Color,     // #5c5346 faint
}

/// Built-in theme names. `by_name` accepts any of these (case-insensitive).
pub const THEME_NAMES: &[&str] = &["captain", "midnight", "paper"];

/// Resolve a theme by name. Unknown names fall back to `captain`.
pub fn by_name(name: &str) -> Theme {
    match name.trim().to_lowercase().as_str() {
        "midnight" => THEME_MIDNIGHT,
        "paper" => THEME_PAPER,
        _ => THEME_CAPTAIN,
    }
}

pub const THEME_CAPTAIN: Theme = Theme {
    brand: Color::Rgb(0xf2, 0xa9, 0x3c),
    coral: Color::Rgb(0xf0, 0x67, 0x4a),
    agent: Color::Rgb(0x4e, 0xc9, 0xb0),
    planner: Color::Rgb(0x6f, 0xa6, 0xe6),
    verifier: Color::Rgb(0xc9, 0xa1, 0x4e),
    add: Color::Rgb(0x74, 0xc2, 0x58),
    rem: Color::Rgb(0xd9, 0x6a, 0x63),
    gold: Color::Rgb(0xf2, 0xc9, 0x4c),
    steel: Color::Rgb(0xb9, 0xb0, 0xa3),
    supervised: Color::Rgb(0x74, 0xc2, 0x58),
    trusted: Color::Rgb(0xf2, 0xa9, 0x3c),
    autonomous: Color::Rgb(0xd9, 0x6a, 0x63),
    bg: Color::Rgb(0x0e, 0x0b, 0x08),
    panel: Color::Rgb(0x16, 0x12, 0x0d),
    line: Color::Rgb(0x2c, 0x25, 0x1c),
    fg: Color::Rgb(0xec, 0xe2, 0xcf),
    dim: Color::Rgb(0x89, 0x7d, 0x6c),
    dimmer: Color::Rgb(0x5c, 0x53, 0x46),
};

/// Cool / dark variant: lower-saturation blues for late-night work.
pub const THEME_MIDNIGHT: Theme = Theme {
    brand: Color::Rgb(0x6f, 0xa6, 0xe6),
    coral: Color::Rgb(0x9b, 0x7e, 0xd1),
    agent: Color::Rgb(0x4e, 0xc9, 0xb0),
    planner: Color::Rgb(0x6f, 0xa6, 0xe6),
    verifier: Color::Rgb(0xc9, 0xa1, 0x4e),
    add: Color::Rgb(0x74, 0xc2, 0x58),
    rem: Color::Rgb(0xd9, 0x6a, 0x63),
    gold: Color::Rgb(0xf2, 0xc9, 0x4c),
    steel: Color::Rgb(0xb9, 0xb0, 0xa3),
    supervised: Color::Rgb(0x74, 0xc2, 0x58),
    trusted: Color::Rgb(0x6f, 0xa6, 0xe6),
    autonomous: Color::Rgb(0xd9, 0x6a, 0x63),
    bg: Color::Rgb(0x06, 0x08, 0x0e),
    panel: Color::Rgb(0x0e, 0x12, 0x1a),
    line: Color::Rgb(0x20, 0x26, 0x33),
    fg: Color::Rgb(0xd6, 0xde, 0xeb),
    dim: Color::Rgb(0x7a, 0x84, 0x99),
    dimmer: Color::Rgb(0x4a, 0x52, 0x63),
};

/// Light variant: cream paper background, dark fg. For bright environments.
pub const THEME_PAPER: Theme = Theme {
    brand: Color::Rgb(0xa6, 0x5a, 0x1a),
    coral: Color::Rgb(0xa6, 0x3a, 0x2a),
    agent: Color::Rgb(0x2f, 0x7d, 0x67),
    planner: Color::Rgb(0x2e, 0x5a, 0x9c),
    verifier: Color::Rgb(0x7a, 0x5e, 0x2c),
    add: Color::Rgb(0x3a, 0x7d, 0x2e),
    rem: Color::Rgb(0xa6, 0x3a, 0x2a),
    gold: Color::Rgb(0xa6, 0x80, 0x1a),
    steel: Color::Rgb(0x5e, 0x55, 0x47),
    supervised: Color::Rgb(0x3a, 0x7d, 0x2e),
    trusted: Color::Rgb(0xa6, 0x5a, 0x1a),
    autonomous: Color::Rgb(0xa6, 0x3a, 0x2a),
    bg: Color::Rgb(0xf3, 0xee, 0xe1),
    panel: Color::Rgb(0xe7, 0xe0, 0xcc),
    line: Color::Rgb(0xc6, 0xbc, 0xa3),
    fg: Color::Rgb(0x2a, 0x24, 0x18),
    dim: Color::Rgb(0x6a, 0x60, 0x4c),
    dimmer: Color::Rgb(0x9a, 0x90, 0x7c),
};

impl Theme {
    pub fn autonomy_color(&self, level: &crate::event::AutonomyLevel) -> Color {
        match level {
            crate::event::AutonomyLevel::Supervised => self.supervised,
            crate::event::AutonomyLevel::Trusted => self.trusted,
            crate::event::AutonomyLevel::Autonomous => self.autonomous,
        }
    }

    pub fn agent_color(&self, role: &str) -> Color {
        match role {
            "planner" => self.planner,
            "coder" => self.agent,
            "verifier" => self.verifier,
            "swarm" => self.gold,
            _ => self.steel,
        }
    }

    pub fn spinner_frame(&self, _index: usize) -> &str {
        const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        FRAMES[_index % FRAMES.len()]
    }

    pub fn flight_verb(&self, index: usize) -> &str {
        const VERBS: &[&str] = &[
            "Soaring", "Gliding", "Diving", "Scouting", "Perching", "Foraging", "Wheeling",
        ];
        VERBS[index % VERBS.len()]
    }
}

impl Default for Theme {
    fn default() -> Self {
        THEME_CAPTAIN
    }
}

// ─── ASCII Logo (§9.4) ──────────────────────────────────────────────────────────

// Validated console mascot (§9.4): two-feather crest, thick eyebrow, open eye +
// pirate patch, coral beak, cheek blush, cream belly, feet + key in the wing.
pub const ASCII_SPARROW: &str = r#"
        ^^
      .-~~~-.
     /__     \
    | o   ██  |
    |    v    |
    | .       |
     \ \__/  /
      '-..-'
      /|  |\  ╤━o
     '_|  |_'
"#;

pub const ASCII_WORDMARK: &str = "S P A R R O W";

pub fn ascii_sparrow_at_frame(frame: u64) -> String {
    let blink = frame % 88 == 0;
    let bob = frame % 60 == 0;
    let mut art = ASCII_SPARROW.to_string();
    if blink {
        // eye line is "| o   ██  |" → close the eye to a dash
        art = art.replace("| o ", "| - ");
    }
    if bob {
        art.lines()
            .map(|line| {
                if line.is_empty() {
                    String::new()
                } else {
                    format!(" {}", line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        art
    }
}

pub fn boot_sequence() -> Vec<String> {
    vec![
        format!("{}", ASCII_SPARROW),
        format!("  {}  ", ASCII_WORDMARK),
        String::new(),
        "one cli · grows with you".to_string(),
        String::new(),
        "boot: router · surfaces · sandbox · skills · memory · autonomy · ready".to_string(),
    ]
}
