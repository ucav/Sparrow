//! Fallback parser for tool calls emitted as inline markup instead of the
//! provider's native function-calling JSON.
//!
//! Some models — notably DeepSeek served through certain OpenAI-compatible
//! proxies (e.g. `opencode-go`) — emit tool calls as XML-ish markup inside the
//! assistant content stream rather than as OpenAI `tool_calls`:
//!
//! ```text
//! <｜｜DSML｜｜tool_calls>
//! <｜｜DSML｜｜invoke name="read">
//! <｜｜DSML｜｜parameter name="file_path" string="true">config.py</｜｜DSML｜｜parameter>
//! </｜｜DSML｜｜invoke>
//! </｜｜DSML｜｜tool_calls>
//! ```
//!
//! Anthropic-style `<invoke name="...">` blocks use the same shape without the
//! `｜｜DSML｜｜` token. When the OpenAI-compatible layer sees this in `content`
//! (with `finish_reason: "stop"`), the tool would otherwise leak to the user as
//! raw text and never execute. We detect and normalize both forms into real
//! tool calls.

use regex::Regex;
use serde_json::{Map, Value};

/// The DeepSeek special token that wraps each tag: `｜｜DSML｜｜` where `｜` is
/// U+FF5C (fullwidth vertical line).
const DSML_TOKEN: &str = "\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}";

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedToolCall {
    pub name: String,
    pub args: Value,
}

/// Cheap pre-check: does this text contain inline tool-call markup we can parse?
/// Used to (a) decide whether to suppress the raw text from the user and
/// (b) whether to run the (more expensive) full extraction at stream end.
pub fn looks_like_tool_markup(text: &str) -> bool {
    (text.contains(DSML_TOKEN) || text.contains("<invoke ") || text.contains("<invoke\t"))
        && text.contains("name=")
}

fn strip_dsml(text: &str) -> String {
    // Remove the exact DSML token so `<｜｜DSML｜｜invoke ...>` becomes
    // `<invoke ...>`. We never touch parameter *values* because the token only
    // appears as a tag prefix.
    text.replace(DSML_TOKEN, "")
}

fn coerce(raw: &str) -> Value {
    let t = raw.trim();
    if t == "true" {
        return Value::Bool(true);
    }
    if t == "false" {
        return Value::Bool(false);
    }
    if let Ok(i) = t.parse::<i64>() {
        return Value::from(i);
    }
    if let Ok(f) = t.parse::<f64>() {
        // Keep integers integral; only use float when it really is one.
        if t.contains('.') {
            return Value::from(f);
        }
    }
    Value::String(t.to_string())
}

/// Extract every `<invoke name="X"> … <parameter name="Y">Z</parameter> … </invoke>`
/// block (tolerating the `｜｜DSML｜｜` token prefix) into structured tool calls.
pub fn extract_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    let cleaned = strip_dsml(text);
    let invoke_re = Regex::new(r#"(?s)<invoke\s+name="([^"]+)"\s*>(.*?)</invoke>"#)
        .expect("static invoke regex");
    let param_re = Regex::new(r#"(?s)<parameter\s+name="([^"]+)"[^>]*?>(.*?)</parameter>"#)
        .expect("static parameter regex");

    let mut calls = Vec::new();
    for inv in invoke_re.captures_iter(&cleaned) {
        let name = inv[1].trim().to_string();
        let body = &inv[2];
        let mut args = Map::new();
        for p in param_re.captures_iter(body) {
            let pname = p[1].trim().to_string();
            let pval = coerce(&p[2]);
            args.insert(pname, pval);
        }
        if !name.is_empty() {
            calls.push(ParsedToolCall {
                name,
                args: Value::Object(args),
            });
        }
    }
    calls
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "<\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}tool_calls>\n<\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}invoke name=\"read\">\n<\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}parameter name=\"file_path\" string=\"true\">config.py</\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}parameter>\n</\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}invoke>\n</\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}tool_calls>";

    #[test]
    fn detects_dsml_markup() {
        assert!(looks_like_tool_markup(SAMPLE));
        assert!(!looks_like_tool_markup(
            "just a normal answer about config.py"
        ));
    }

    #[test]
    fn parses_dsml_single_tool() {
        let calls = extract_tool_calls(SAMPLE);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].args["file_path"], "config.py");
    }

    #[test]
    fn parses_anthropic_style_without_dsml() {
        let text = r#"<invoke name="fs_write">
<parameter name="path">reverse.py</parameter>
<parameter name="content">def f(): pass</parameter>
</invoke>"#;
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "fs_write");
        assert_eq!(calls[0].args["path"], "reverse.py");
        assert_eq!(calls[0].args["content"], "def f(): pass");
    }

    #[test]
    fn parses_multiple_invokes() {
        let text = r#"<invoke name="a"><parameter name="x">1</parameter></invoke>
<invoke name="b"><parameter name="y">two</parameter></invoke>"#;
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "a");
        assert_eq!(calls[0].args["x"], 1);
        assert_eq!(calls[1].name, "b");
        assert_eq!(calls[1].args["y"], "two");
    }

    #[test]
    fn ignores_plain_text() {
        assert!(extract_tool_calls("no tools here, just prose").is_empty());
    }
}
