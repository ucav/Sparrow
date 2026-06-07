// ─── Table formatting ─────────────────────────────────────────────────────────
// Auto-sizes columns based on content, aligns text, adds bold header row with
// separator, and handles terminal width overflow by truncating cells.

/// ANSI style constants for table rendering.
pub struct TableStyles {
    pub header: &'static str,
    pub cell: &'static str,
    pub separator: &'static str,
    pub border: &'static str,
    pub reset: &'static str,
}

impl Default for TableStyles {
    fn default() -> Self {
        Self {
            header: "\x1b[1;38;2;242;169;60m", // bold amber
            cell: "\x1b[38;2;236;226;207m",    // normal fg
            separator: "\x1b[38;2;92;83;70m",  // dimmer
            border: "\x1b[38;2;92;83;70m",     // dimmer
            reset: "\x1b[0m",
        }
    }
}

/// Alignment for table columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Configuration for a table column.
#[derive(Debug, Clone)]
pub struct ColumnConfig {
    pub header: String,
    pub align: Align,
    pub min_width: usize,
    pub max_width: Option<usize>,
}

impl ColumnConfig {
    pub fn new(header: &str) -> Self {
        Self {
            header: header.to_string(),
            align: Align::Left,
            min_width: header.len(),
            max_width: None,
        }
    }

    pub fn with_align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
}

/// Render a table as ANSI-formatted terminal text.
///
/// `headers` — column headers (displayed in bold)
/// `rows` — data rows, each being a Vec of cell strings
/// `max_width` — optional terminal width to constrain total table width
/// `styles` — optional custom styles
///
/// Auto-sizes columns based on content. Truncates cells that exceed their
/// computed column width. Adds separator lines between header and body.
pub fn render_table(
    headers: &[&str],
    rows: &[Vec<String>],
    max_width: Option<usize>,
    styles: Option<&TableStyles>,
) -> String {
    if headers.is_empty() {
        return String::new();
    }

    let styles = styles.unwrap_or(&TABLE_STYLES_DEFAULT);
    let col_count = headers.len();

    // Compute initial column widths from headers
    let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

    // Expand based on cell content
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i >= col_count {
                break;
            }
            let cell_len = cell.chars().count();
            if cell_len > col_widths[i] {
                col_widths[i] = cell_len;
            }
        }
    }

    // Apply minimum width of 3
    for w in col_widths.iter_mut() {
        *w = (*w).max(3);
    }

    // If max_width is specified, scale down columns proportionally
    if let Some(max_w) = max_width {
        let separator_width = (col_count + 1) * 3; // " │ " separators
        let total_content: usize = col_widths.iter().sum::<usize>() + separator_width;
        if total_content > max_w {
            // Scale down proportionally
            let available = max_w.saturating_sub(separator_width);
            let scale = available as f64 / (total_content - separator_width) as f64;
            for w in col_widths.iter_mut() {
                *w = ((*w as f64) * scale).max(3.0) as usize;
            }
        }
    }

    // Build the table
    let mut out = String::new();

    // Top border
    render_horizontal_line(&mut out, &col_widths, &styles);

    // Header row
    let header_cells: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
    render_row(&mut out, &header_cells, &col_widths, &styles, true);

    // Header/body separator
    render_separator(&mut out, &col_widths, &styles);

    // Data rows
    for row in rows {
        let cells: Vec<String> = row.iter().map(|c| c.to_string()).collect();
        render_row(&mut out, &cells, &col_widths, &styles, false);
    }

    // Bottom border
    render_horizontal_line(&mut out, &col_widths, &styles);

    out
}

/// Render a table using column configs for fine-grained control.
pub fn render_table_with_config(
    columns: &[ColumnConfig],
    rows: &[Vec<String>],
    max_width: Option<usize>,
    styles: Option<&TableStyles>,
) -> String {
    let headers: Vec<&str> = columns.iter().map(|c| c.header.as_str()).collect();
    render_table(&headers, rows, max_width, styles)
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

static TABLE_STYLES_DEFAULT: TableStyles = TableStyles {
    header: "\x1b[1;38;2;242;169;60m",
    cell: "\x1b[38;2;236;226;207m",
    separator: "\x1b[38;2;92;83;70m",
    border: "\x1b[38;2;92;83;70m",
    reset: "\x1b[0m",
};

fn render_horizontal_line(out: &mut String, widths: &[usize], s: &TableStyles) {
    out.push_str(s.border);
    for &w in widths {
        out.push('├');
        out.push_str(&"─".repeat(w + 2));
    }
    out.push('┤');
    out.push_str(s.reset);
    out.push('\n');
}

fn render_separator(out: &mut String, widths: &[usize], s: &TableStyles) {
    out.push_str(s.border);
    for &w in widths {
        out.push('├');
        out.push_str(&"─".repeat(w + 2));
    }
    out.push('┤');
    out.push_str(s.reset);
    out.push('\n');
}

fn render_row(
    out: &mut String,
    cells: &[String],
    widths: &[usize],
    s: &TableStyles,
    is_header: bool,
) {
    let style = if is_header { s.header } else { s.cell };

    out.push_str(s.border);
    out.push('│');

    for (i, cell) in cells.iter().enumerate() {
        if i >= widths.len() {
            break;
        }
        let w = widths[i];
        let display = truncate_cell(cell, w);
        out.push(' ');
        out.push_str(style);
        out.push_str(&display);
        // Pad with spaces
        let display_len = display.chars().count();
        if display_len < w {
            out.push_str(&" ".repeat(w - display_len));
        }
        out.push_str(s.reset);
        out.push(' ');
        out.push_str(s.border);
        out.push('│');
    }

    // If we have fewer cells than columns, add empty cells
    for i in cells.len()..widths.len() {
        let w = widths[i];
        out.push(' ');
        out.push_str(&" ".repeat(w));
        out.push(' ');
        out.push_str(s.border);
        out.push('│');
    }

    out.push_str(s.reset);
    out.push('\n');
}

/// Truncate a cell to fit within the given width, adding an ellipsis if needed.
fn truncate_cell(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(width - 1).collect();
    out.push('…');
    out
}
