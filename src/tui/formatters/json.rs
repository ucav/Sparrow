//! Simple JSON pretty-printer with ANSI colors.
//! No external deps beyond std.

pub struct JsonColors {
    pub key: &'static str,
    pub string: &'static str,
    pub number: &'static str,
    pub boolean: &'static str,
    pub null: &'static str,
    pub bracket: &'static str,
    pub reset: &'static str,
}

impl Default for JsonColors {
    fn default() -> Self {
        Self {
            key: "\x1b[38;2;111;166;230m",
            string: "\x1b[38;2;116;194;88m",
            number: "\x1b[38;2;242;201;76m",
            boolean: "\x1b[38;2;86;182;194m",
            null: "\x1b[38;2;137;125;108m",
            bracket: "\x1b[38;2;185;176;163m",
            reset: "\x1b[0m",
        }
    }
}

/// Pretty-print a JSON string with ANSI colors.
pub fn format_json(json_str: &str) -> String {
    let colors = JsonColors::default();
    // Use serde_json to parse and re-serialize with pretty print
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(value) => {
            let pretty = serde_json::to_string_pretty(&value).unwrap_or_else(|_| json_str.to_string());
            colorize_json(&pretty, &colors)
        }
        Err(_) => {
            // Not valid JSON — return as-is
            json_str.to_string()
        }
    }
}

/// Apply basic coloring to already-pretty-printed JSON.
fn colorize_json(json: &str, c: &JsonColors) -> String {
    let mut out = String::with_capacity(json.len() + 256);
    let mut in_string = false;
    let mut after_colon = false;
    let bytes = json.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let ch = bytes[i] as char;

        match ch {
            '"' if !in_string => {
                in_string = true;
                // Check if this looks like a key (next non-whitespace is ':')
                let rest = &json[i + 1..];
                let key_end = rest.find('"').unwrap_or(rest.len());
                let after_key = &rest[key_end + 1..];
                let is_key = after_key.trim_start().starts_with(':');
                if is_key {
                    out.push_str(c.key);
                } else {
                    out.push_str(c.string);
                }
                out.push('"');
            }
            '"' if in_string => {
                out.push('"');
                out.push_str(c.reset);
                in_string = false;
                after_colon = false;
            }
            ':' if !in_string => {
                out.push(':');
                out.push(' ');
                after_colon = true;
            }
            't' | 'f' if !in_string && !after_colon => {
                out.push_str(c.boolean);
                out.push(ch);
            }
            'n' if !in_string && !after_colon => {
                out.push_str(c.null);
                out.push(ch);
            }
            '0'..='9' | '-' if !in_string && after_colon => {
                out.push_str(c.number);
                out.push(ch);
                after_colon = false;
            }
            '{' | '}' | '[' | ']' if !in_string => {
                out.push_str(c.bracket);
                out.push(ch);
                out.push_str(c.reset);
                after_colon = false;
            }
            _ => {
                out.push(ch);
                if ch == ',' && !in_string {
                    after_colon = false;
                }
                if ch == '\n' {
                    after_colon = false;
                }
            }
        }
        i += 1;
    }

    if in_string {
        out.push_str(c.reset);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_json() {
        let input = r#"{"name":"test","count":42,"active":true}"#;
        let output = format_json(input);
        assert!(output.contains("name"));
        assert!(output.contains("42"));
    }
}
