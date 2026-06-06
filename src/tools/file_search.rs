//! File search tool for Sparrow.
//!
//! Fast file content search using ripgrep (rg) with fallback to grep.
//! Returns formatted results with file paths, line numbers, and context.

use std::path::Path;
use std::process::Command;

/// Search result for a single match.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub file: String,
    pub line_number: u64,
    pub content: String,
}

/// Search options.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// File glob pattern (e.g., "*.rs", "*.py")
    pub file_glob: Option<String>,
    /// Max results to return
    pub max_results: usize,
    /// Case insensitive search
    pub case_insensitive: bool,
    /// Show N lines of context around matches
    pub context_lines: usize,
    /// Only match whole words
    pub whole_word: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            file_glob: None,
            max_results: 50,
            case_insensitive: true,
            context_lines: 0,
            whole_word: false,
        }
    }
}

/// Search for a pattern in files.
///
/// Uses ripgrep if available, falls back to grep.
pub fn search_files(
    pattern: &str,
    directory: &Path,
    options: &SearchOptions,
) -> anyhow::Result<Vec<SearchMatch>> {
    if rg_available() {
        search_with_rg(pattern, directory, options)
    } else {
        search_with_grep(pattern, directory, options)
    }
}

/// Check if ripgrep is available.
fn rg_available() -> bool {
    Command::new("rg")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Search using ripgrep.
fn search_with_rg(
    pattern: &str,
    directory: &Path,
    options: &SearchOptions,
) -> anyhow::Result<Vec<SearchMatch>> {
    let mut cmd = Command::new("rg");

    cmd.arg("--line-number");
    cmd.arg("--no-heading");
    cmd.arg("--color=never");

    if options.case_insensitive {
        cmd.arg("--ignore-case");
    }

    if options.whole_word {
        cmd.arg("--word-regexp");
    }

    if options.context_lines > 0 {
        cmd.arg("-C")
            .arg(options.context_lines.to_string());
    }

    if let Some(ref glob) = options.file_glob {
        cmd.arg("--glob").arg(glob);
    }

    cmd.arg("--").arg(pattern);
    cmd.arg(directory);

    let output = cmd.output()?;

    if !output.status.success() {
        // rg returns 1 when no matches found — not an error
        let code = output.status.code().unwrap_or(0);
        if code == 1 {
            return Ok(Vec::new());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ripgrep failed: {stderr}");
    }

    parse_rg_output(&String::from_utf8_lossy(&output.stdout), options.max_results)
}

/// Parse ripgrep output into SearchMatch structs.
fn parse_rg_output(output: &str, max_results: usize) -> anyhow::Result<Vec<SearchMatch>> {
    let mut matches = Vec::new();

    for line in output.lines().take(max_results) {
        // rg output format: file.rs:42:matched line content
        if let Some((file_part, rest)) = line.split_once(':') {
            if let Some((line_num_str, content)) = rest.split_once(':') {
                if let Ok(line_number) = line_num_str.parse::<u64>() {
                    matches.push(SearchMatch {
                        file: file_part.to_string(),
                        line_number,
                        content: content.trim().to_string(),
                    });
                }
            }
        }
    }

    Ok(matches)
}

/// Fallback search using grep.
fn search_with_grep(
    pattern: &str,
    directory: &Path,
    options: &SearchOptions,
) -> anyhow::Result<Vec<SearchMatch>> {
    let mut cmd = Command::new("grep");

    cmd.arg("-rn"); // recursive, line numbers
    cmd.arg("--color=never");

    if options.case_insensitive {
        cmd.arg("-i");
    }

    if options.whole_word {
        cmd.arg("-w");
    }

    if options.context_lines > 0 {
        cmd.arg("-C")
            .arg(options.context_lines.to_string());
    }

    if let Some(ref glob) = options.file_glob {
        cmd.arg("--include").arg(glob);
    }

    cmd.arg(pattern);
    cmd.arg(directory);

    let output = cmd.output()?;

    // grep returns 1 when no matches
    let code = output.status.code().unwrap_or(0);
    if code > 1 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("grep failed: {stderr}");
    }

    parse_grep_output(&String::from_utf8_lossy(&output.stdout), options.max_results)
}

/// Parse grep output.
fn parse_grep_output(output: &str, max_results: usize) -> anyhow::Result<Vec<SearchMatch>> {
    // Same format as rg: file:line:content
    parse_rg_output(output, max_results)
}

/// Search for files by name pattern (glob).
pub fn find_files(pattern: &str, directory: &Path) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();

    // Use fd if available, else find
    if fd_available() {
        let output = Command::new("fd")
            .arg("--type").arg("f")
            .arg(pattern)
            .arg(directory)
            .output()?;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            files.push(line.to_string());
        }
    } else {
        let output = Command::new("find")
            .arg(directory)
            .arg("-name").arg(pattern)
            .arg("-type").arg("f")
            .output()?;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            files.push(line.to_string());
        }
    }

    files.sort();
    Ok(files)
}

fn fd_available() -> bool {
    Command::new("fd")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rg_output() {
        let output = "src/main.rs:42:fn main() {\nsrc/lib.rs:10:pub mod tools;\n";
        let matches = parse_rg_output(output, 10).unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].file, "src/main.rs");
        assert_eq!(matches[0].line_number, 42);
        assert_eq!(matches[1].content, "pub mod tools;");
    }
}
