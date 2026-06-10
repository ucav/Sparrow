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
///
/// B3: require a COMPLETE structure (opening *and* closing tag), not just two
/// loose substrings. Prose that merely mentions `<invoke name="…">` (a code
/// review, documentation) must NOT be suppressed and re-parsed as phantom
/// tool calls.
pub fn looks_like_tool_markup(text: &str) -> bool {
    let stripped = strip_dsml(text);
    // Anthropic-style / DSML `<invoke …>…</invoke>` block, fully closed.
    let invoke_closed = (stripped.contains("<invoke ") || stripped.contains("<invoke\t"))
        && stripped.contains("name=")
        && stripped.contains("</invoke>");
    let trimmed = stripped.trim();
    let fenced_json = trimmed.starts_with("```json")
        && trimmed.ends_with("```")
        && (stripped.contains("\"arguments\"") || stripped.contains("\"args\""));
    let bracketed_tool = stripped.contains("[TOOL_CALL]") && stripped.contains("[/TOOL_CALL]");
    let deepseek_tool =
        stripped.contains("<｜tool▁call▁begin｜>") && stripped.contains("<｜tool▁call▁end｜>");
    invoke_closed || fenced_json || bracketed_tool || deepseek_tool
}

/// True while the first streamed content bytes could still become one of the
/// supported tool-call markup forms. Used by streaming adapters to avoid
/// leaking partial DSML/DeepSeek tags before `looks_like_tool_markup()` can see
/// a complete closed block.
pub fn could_be_tool_markup_prefix(text: &str) -> bool {
    let stripped = strip_dsml(text);
    let trimmed = stripped.trim_start();
    if trimmed.is_empty() {
        return true;
    }
    const STARTS: &[&str] = &[
        "<invoke",
        "[TOOL_CALL]",
        "```json",
        "<｜tool▁call",
        "<｜tool▁calls",
    ];
    STARTS
        .iter()
        .any(|start| start.starts_with(trimmed) || trimmed.starts_with(start))
}

fn strip_dsml(text: &str) -> String {
    // Remove the exact DSML token so `<｜｜DSML｜｜invoke ...>` becomes
    // `<invoke ...>`. We never touch parameter *values* because the token only
    // appears as a tag prefix.
    text.replace(DSML_TOKEN, "")
}

/// Coerce a parameter's raw text into a JSON value.
///
/// B2: when the markup explicitly declares the value is a string
/// (`string="true"`, as DSML emits), NEVER coerce to number/bool — `"123"`
/// must stay the string "123". And NEVER `trim()` a string value: a file's
/// `content` keeps its leading/trailing whitespace and newlines, which trim
/// would silently corrupt.
fn coerce(raw: &str, declared_string: bool) -> Value {
    if declared_string {
        return Value::String(raw.to_string());
    }
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
    // Untyped string value: preserve verbatim (no trim) — see B2.
    Value::String(raw.to_string())
}

/// Extract supported inline tool-call formats into structured tool calls.
///
/// I4: keep this as a small parser registry. The first parser that finds calls
/// wins, so a prose block containing examples from another format cannot mix
/// accidental partial matches into the real call.
pub fn extract_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    for parser in [
        parse_invoke_tool_calls as fn(&str) -> Vec<ParsedToolCall>,
        parse_tool_call_blocks,
        parse_json_fences,
        parse_deepseek_tool_calls,
    ] {
        let calls = parser(text);
        if !calls.is_empty() {
            return calls;
        }
    }
    Vec::new()
}

fn parse_invoke_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    let cleaned = strip_dsml(text);
    let invoke_re = Regex::new(r#"(?s)<invoke\s+name="([^"]+)"\s*>(.*?)</invoke>"#)
        .expect("static invoke regex");
    // Capture the attribute blob (group 2) so we can honour `string="true"`.
    let param_re = Regex::new(r#"(?s)<parameter\s+name="([^"]+)"([^>]*)>(.*?)</parameter>"#)
        .expect("static parameter regex");

    let mut calls = Vec::new();
    for inv in invoke_re.captures_iter(&cleaned) {
        let name = inv[1].trim().to_string();
        let body = &inv[2];
        let mut args = Map::new();
        for p in param_re.captures_iter(body) {
            let pname = p[1].trim().to_string();
            let declared_string = p[2].contains("string=\"true\"");
            let pval = coerce(&p[3], declared_string);
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

fn parse_tool_call_blocks(text: &str) -> Vec<ParsedToolCall> {
    let block_re = Regex::new(r#"(?s)\[TOOL_CALL\](.*?)\[/TOOL_CALL\]"#)
        .expect("static tool-call block regex");
    block_re
        .captures_iter(text)
        .filter_map(|cap| serde_json::from_str::<Value>(cap[1].trim()).ok())
        .filter_map(call_from_json)
        .collect()
}

fn parse_json_fences(text: &str) -> Vec<ParsedToolCall> {
    let trimmed = text.trim();
    if !trimmed.starts_with("```json") || !trimmed.ends_with("```") {
        return Vec::new();
    }
    let fence_re = Regex::new(r#"(?s)```json\s*(.*?)\s*```"#).expect("static json fence regex");
    fence_re
        .captures_iter(text)
        .filter_map(|cap| serde_json::from_str::<Value>(cap[1].trim()).ok())
        .flat_map(calls_from_json_value)
        .collect()
}

fn parse_deepseek_tool_calls(text: &str) -> Vec<ParsedToolCall> {
    let call_re = Regex::new(r#"(?s)<｜tool▁call▁begin｜>(.*?)<｜tool▁call▁end｜>"#)
        .expect("static deepseek tool-call regex");
    call_re
        .captures_iter(text)
        .filter_map(|cap| {
            let body = cap[1].trim();
            let (maybe_name, json_text) = match body.split_once("<｜tool▁sep｜>") {
                Some((name, json)) => (Some(name.trim()), json.trim()),
                None => (None, body),
            };
            let value = serde_json::from_str::<Value>(json_text).ok()?;
            if let Some(call) = call_from_json(value.clone()) {
                return Some(call);
            }
            let name = maybe_name?.trim();
            if name.is_empty() || name == "function" {
                return None;
            }
            Some(ParsedToolCall {
                name: name.to_string(),
                args: value,
            })
        })
        .collect()
}

fn calls_from_json_value(value: Value) -> Vec<ParsedToolCall> {
    match value {
        Value::Array(items) => items.into_iter().filter_map(call_from_json).collect(),
        other => call_from_json(other).into_iter().collect(),
    }
}

fn call_from_json(value: Value) -> Option<ParsedToolCall> {
    let obj = value.as_object()?;

    if let Some(function) = obj.get("function").and_then(Value::as_object) {
        let name = function.get("name").and_then(Value::as_str)?;
        let args = function
            .get("arguments")
            .cloned()
            .or_else(|| obj.get("arguments").cloned())
            .unwrap_or_else(|| Value::Object(Map::new()));
        return Some(ParsedToolCall {
            name: name.to_string(),
            args: normalize_args(args),
        });
    }

    let name = obj
        .get("name")
        .or_else(|| obj.get("tool"))
        .or_else(|| obj.get("tool_name"))
        .and_then(Value::as_str)?;
    let args = obj
        .get("arguments")
        .or_else(|| obj.get("args"))
        .or_else(|| obj.get("input"))
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    Some(ParsedToolCall {
        name: name.to_string(),
        args: normalize_args(args),
    })
}

fn normalize_args(args: Value) -> Value {
    match args {
        Value::String(s) => serde_json::from_str::<Value>(&s).unwrap_or(Value::String(s)),
        other => other,
    }
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

    #[test]
    fn i4_parses_json_tool_call_fence() {
        let text = r#"```json
{"name":"fs_write","arguments":{"path":"poeme.txt","content":"salut"}}
```"#;
        assert!(looks_like_tool_markup(text));
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "fs_write");
        assert_eq!(calls[0].args["path"], "poeme.txt");
        assert_eq!(calls[0].args["content"], "salut");
    }

    #[test]
    fn i4_does_not_parse_embedded_json_example_as_tool_call() {
        let text = r#"Here is the format:
```json
{"name":"read","arguments":{"file_path":"config.py"}}
```
Use it carefully."#;
        assert!(!looks_like_tool_markup(text));
        assert!(extract_tool_calls(text).is_empty());
    }

    #[test]
    fn i4_parses_bracketed_tool_call() {
        let text =
            r#"[TOOL_CALL]{"name":"read","arguments":{"file_path":"config.py"}}[/TOOL_CALL]"#;
        assert!(looks_like_tool_markup(text));
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].args["file_path"], "config.py");
    }

    #[test]
    fn i4_parses_deepseek_native_tool_call_json() {
        let text = r#"<｜tool▁calls▁begin｜><｜tool▁call▁begin｜>{"name":"read","arguments":{"file_path":"src/main.rs"}}<｜tool▁call▁end｜><｜tool▁calls▁end｜>"#;
        assert!(looks_like_tool_markup(text));
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].args["file_path"], "src/main.rs");
    }

    #[test]
    fn i4_parses_deepseek_native_tool_call_with_separator() {
        let text = r#"<｜tool▁call▁begin｜>fs_write<｜tool▁sep｜>{"path":"a.txt","content":"ok"}<｜tool▁call▁end｜>"#;
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "fs_write");
        assert_eq!(calls[0].args["path"], "a.txt");
    }

    #[test]
    fn i4_parses_openai_function_shape_with_string_arguments() {
        let text = r#"```json
{"function":{"name":"read","arguments":"{\"file_path\":\"Cargo.toml\"}"}}
```"#;
        let calls = extract_tool_calls(text);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].args["file_path"], "Cargo.toml");
    }

    #[test]
    fn b1_detects_partial_tool_markup_prefixes() {
        assert!(could_be_tool_markup_prefix("<"));
        assert!(could_be_tool_markup_prefix("<invoke name=\"read\""));
        assert!(could_be_tool_markup_prefix("[TOOL"));
        assert!(could_be_tool_markup_prefix("```json\n{\"name\""));
        assert!(could_be_tool_markup_prefix("<｜tool▁call▁begin｜>"));
        assert!(!could_be_tool_markup_prefix(
            "Bonjour <invoke name=\"read\">"
        ));
        assert!(!could_be_tool_markup_prefix("plain text"));
    }

    #[test]
    fn b3_prose_mentioning_invoke_is_not_treated_as_markup() {
        // A code review or doc that *talks about* the markup must not be
        // suppressed and re-parsed as a phantom tool call.
        let prose = r#"To call a tool, the model emits `<invoke name="read">` —
note there is no closing tag in this explanation."#;
        assert!(!looks_like_tool_markup(prose));
    }

    #[test]
    fn b3_complete_block_is_detected() {
        let t = r#"<invoke name="read"><parameter name="p">x</parameter></invoke>"#;
        assert!(looks_like_tool_markup(t));
    }

    #[test]
    fn b2_declared_string_is_not_coerced() {
        // DSML marks values string="true"; "123" must stay a string.
        let t = "<\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}invoke name=\"x\">\n<\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}parameter name=\"n\" string=\"true\">123</\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}parameter>\n</\u{FF5C}\u{FF5C}DSML\u{FF5C}\u{FF5C}invoke>";
        let calls = extract_tool_calls(t);
        assert_eq!(calls[0].args["n"], Value::String("123".into()));
    }

    #[test]
    fn b2_file_content_whitespace_is_preserved() {
        // A file's content keeps leading/trailing newlines — trim would corrupt it.
        let t = "<invoke name=\"fs_write\"><parameter name=\"content\">\nline1\nline2\n</parameter></invoke>";
        let calls = extract_tool_calls(t);
        assert_eq!(
            calls[0].args["content"],
            Value::String("\nline1\nline2\n".into())
        );
    }
}
