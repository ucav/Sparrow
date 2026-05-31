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
