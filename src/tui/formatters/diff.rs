// ─── Diff formatting ──────────────────────────────────────────────────────────
// Parses unified diff format and emits ANSI-coloured terminal output.
// + lines are green, - lines are red, @@ headers are cyan, file headers are bold.

/// A single line in a diff hunk with its kind.
#[derive(Debug, Clone)]
pub enum DiffLineKind {
    Context,
    Plus,
    Minus,
    Hunk,
    FileHeader,
}

/// One line of a diff.
#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

/// A parsed diff with optional file header and hunks.
#[derive(Debug, Clone)]
pub struct ParsedDiff {
    pub file_header: Option<String>,
    pub lines: Vec<DiffLine>,
    pub plus_count: u32,
    pub minus_count: u32,
}

/// ANSI colour constants for diff rendering.
pub struct DiffColors {
    pub plus: &'static str,
    pub minus: &'static str,
    pub hunk: &'static str,
    pub file: &'static str,
    pub context: &'static str,
    pub reset: &'static str,
}

impl Default for DiffColors {
    fn default() -> Self {
        Self {
            plus: "\x1b[38;2;116;194;88m",   // green
            minus: "\x1b[38;2;217;106;99m",  // red
            hunk: "\x1b[38;2;86;182;194m",   // cyan
            file: "\x1b[1;38;2;242;169;60m", // bold amber
            context: "\x1b[38;2;137;125;108m", // dim
            reset: "\x1b[0m",
        }
    }
}

/// Parse a unified diff string into structured lines.
///
/// Handles the standard unified diff format:
/// ```
/// diff --git a/file b/file
/// --- a/file
/// +++ b/file
/// @@ -1,3 +1,4 @@
///  context
/// -removed
/// +added
/// ```
pub fn parse(diff: &str) -> ParsedDiff {
    let mut lines = Vec::new();
    let mut file_header: Option<String> = None;
    let mut plus_count: u32 = 0;
    let mut minus_count: u32 = 0;

    for raw in diff.lines() {
        let line = raw.to_string();

        if raw.starts_with("diff --git") || raw.starts_with("--- ") || raw.starts_with("+++ ") {
            if file_header.is_none() && (raw.starts_with("diff --git") || raw.starts_with("--- ")) {
                // Extract filename
                let fname = if raw.starts_with("diff --git") {
                    raw.strip_prefix("diff --git a/")
                        .and_then(|s| s.split_whitespace().next())
                        .unwrap_or(raw)
                } else if raw.starts_with("--- ") {
                    raw.strip_prefix("--- a/")
                        .or_else(|| raw.strip_prefix("--- "))
                        .unwrap_or(raw)
                } else {
                    raw
                };
                file_header = Some(fname.to_string());
            }
            lines.push(DiffLine {
                kind: DiffLineKind::FileHeader,
                text: line,
            });
        } else if raw.starts_with("@@") {
            lines.push(DiffLine {
                kind: DiffLineKind::Hunk,
                text: line,
            });
        } else if raw.starts_with('+') && !raw.starts_with("+++") {
            plus_count += 1;
            lines.push(DiffLine {
                kind: DiffLineKind::Plus,
                text: line,
            });
        } else if raw.starts_with('-') && !raw.starts_with("---") {
            minus_count += 1;
            lines.push(DiffLine {
                kind: DiffLineKind::Minus,
                text: line,
            });
        } else {
            lines.push(DiffLine {
                kind: DiffLineKind::Context,
                text: line,
            });
        }
    }

    ParsedDiff {
        file_header,
        lines,
        plus_count,
        minus_count,
    }
}

/// Render a parsed diff to ANSI-coloured terminal text.
pub fn render(parsed: &ParsedDiff, colors: &DiffColors) -> String {
    let mut out = String::new();

    // Summary line
    if let Some(ref file) = parsed.file_header {
        out.push_str(&format!(
            "{file}📄 {file}{reset}  ",
            file = colors.file,
            reset = colors.reset,
        ));
    }
    out.push_str(&format!(
        "{plus}+{}{reset}  {minus}-{}{reset}\n",
        parsed.plus_count,
        parsed.minus_count,
        plus = colors.plus,
        minus = colors.minus,
        reset = colors.reset
    ));

    // Divider
    out.push_str(&format!(
        "{dim}─────────────────────────────────────────────{reset}\n",
        dim = colors.context,
        reset = colors.reset
    ));

    for entry in &parsed.lines {
        let (prefix, reset) = match entry.kind {
            DiffLineKind::FileHeader => (colors.file, colors.reset),
            DiffLineKind::Hunk => (colors.hunk, colors.reset),
            DiffLineKind::Plus => (colors.plus, colors.reset),
            DiffLineKind::Minus => (colors.minus, colors.reset),
            DiffLineKind::Context => (colors.context, colors.reset),
        };
        out.push_str(&format!("{prefix}{text}{reset}\n", text = entry.text));
    }

    out
}

/// One-shot: parse a unified diff string and render it with colours.
pub fn format_diff(diff: &str) -> String {
    let colors = DiffColors::default();
    let parsed = parse(diff);
    render(&parsed, &colors)
}
