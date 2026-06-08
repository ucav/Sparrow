//! ANSI-to-Ratatui bridge for TUI rendering.
//! Parses ANSI SGR codes and converts to Ratatui Spans.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Parse ANSI-escaped text into Ratatui Spans.
pub fn ansi_to_spans(text: &str, base_style: Style) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut style = base_style;
    let mut i = 0;
    let bytes = text.as_bytes();

    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            if !current.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            i += 2;
            let mut params = String::new();
            while i < bytes.len() && bytes[i] != b'm' {
                params.push(bytes[i] as char);
                i += 1;
            }
            i += 1;
            style = apply_sgr_codes(&params, style);
        } else if bytes[i] == b'\n' {
            if !current.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut current), style));
            }
            spans.push(Span::styled("\n".to_string(), base_style));
            i += 1;
        } else {
            current.push(bytes[i] as char);
            i += 1;
        }
    }

    if !current.is_empty() {
        spans.push(Span::styled(current, style));
    }

    spans
}

fn apply_sgr_codes(params: &str, mut style: Style) -> Style {
    for code in params.split(';') {
        style = apply_sgr(code, style);
    }
    style
}

fn apply_sgr(code: &str, mut style: Style) -> Style {
    match code {
        "0" | "" => Style::default(),
        "1" => Style::default().add_modifier(Modifier::BOLD),
        "2" => Style::default().add_modifier(Modifier::DIM),
        "3" => Style::default().add_modifier(Modifier::ITALIC),
        "4" => Style::default().add_modifier(Modifier::UNDERLINED),
        "30" => Style::default().fg(Color::Black),
        "31" => Style::default().fg(Color::Red),
        "32" => Style::default().fg(Color::Green),
        "33" => Style::default().fg(Color::Yellow),
        "34" => Style::default().fg(Color::Blue),
        "35" => Style::default().fg(Color::Magenta),
        "36" => Style::default().fg(Color::Cyan),
        "37" => Style::default().fg(Color::White),
        "90" => Style::default().fg(Color::Gray),
        "91" => Style::default().fg(Color::LightRed),
        "92" => Style::default().fg(Color::LightGreen),
        "93" => Style::default().fg(Color::LightYellow),
        "94" => Style::default().fg(Color::LightBlue),
        "95" => Style::default().fg(Color::LightMagenta),
        "96" => Style::default().fg(Color::LightCyan),
        _ if code.starts_with("38;2;") => {
            let parts: Vec<&str> = code.split(';').collect();
            if parts.len() >= 5 {
                if let (Ok(r), Ok(g), Ok(b)) = (
                    parts[2].parse::<u8>(),
                    parts[3].parse::<u8>(),
                    parts[4].parse::<u8>(),
                ) {
                    return Style::default().fg(Color::Rgb(r, g, b));
                }
            }
            style
        }
        _ => style,
    }
}

/// Convert rendered text (may contain ANSI) into a Ratatui Line.
pub fn render_line(text: &str, base_style: Style) -> Line<'static> {
    let spans = ansi_to_spans(text, base_style);
    if spans.is_empty() {
        Line::from("")
    } else {
        Line::from(spans)
    }
}
