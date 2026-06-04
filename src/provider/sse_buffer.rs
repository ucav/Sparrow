//! Reassemble newline-delimited frames across arbitrary network chunks.
//!
//! Provider streams (SSE for OpenAI/Anthropic, NDJSON for Ollama) deliver
//! "one frame per line". A single TCP/HTTP chunk can split a frame at any byte,
//! which the previous code did not handle — it ran `text.lines()` on each
//! chunk in isolation and silently dropped the trailing partial line, eating
//! characters mid-stream (e.g. "à rebours" arrived as "àours" because the
//! "reb" segment lived on the boundary).
//!
//! `LineBuffer` is the minimal fix: feed it chunk bytes, get back the complete
//! lines that have accumulated. The unfinished tail stays inside the buffer
//! until the next chunk completes it.

/// Accumulates raw bytes and yields complete `\n`-terminated lines. Partial
/// trailing data is preserved across calls. Tolerates `\r\n` and mixed UTF-8
/// by accumulating bytes first and only converting to `&str` per complete line.
#[derive(Default)]
pub struct LineBuffer {
    buf: Vec<u8>,
}

impl LineBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `bytes` and return every complete line that is now available.
    /// The returned strings have any trailing `\r` stripped and do NOT include
    /// the terminating `\n`.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        // Find each newline and split.
        let mut start = 0usize;
        let mut i = 0usize;
        while i < self.buf.len() {
            if self.buf[i] == b'\n' {
                let end = if i > start && self.buf[i - 1] == b'\r' {
                    i - 1
                } else {
                    i
                };
                // String::from_utf8_lossy here is intentional: if the LLM ever
                // sends a UTF-8 character split across chunks we'd see U+FFFD,
                // but at the byte level the entire char now sits on this side
                // of the newline (because UTF-8 multibyte sequences never
                // contain 0x0A), so this stays lossless for valid UTF-8.
                out.push(String::from_utf8_lossy(&self.buf[start..end]).into_owned());
                start = i + 1;
            }
            i += 1;
        }
        if start > 0 {
            // Drop emitted bytes, keep the unfinished tail.
            self.buf.drain(..start);
        }
        out
    }

    /// Drain anything left without requiring a final newline. Useful when the
    /// upstream closes without flushing a trailing `\n`.
    pub fn take_remaining(&mut self) -> Option<String> {
        if self.buf.is_empty() {
            return None;
        }
        let s = String::from_utf8_lossy(&self.buf).into_owned();
        self.buf.clear();
        if s.is_empty() { None } else { Some(s) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_line_on_one_push_returns_it() {
        let mut b = LineBuffer::new();
        let lines = b.push(b"hello\n");
        assert_eq!(lines, vec!["hello".to_string()]);
        assert!(b.take_remaining().is_none());
    }

    #[test]
    fn split_line_is_reassembled_across_pushes() {
        let mut b = LineBuffer::new();
        assert!(b.push(b"hel").is_empty());
        assert!(b.push(b"lo wor").is_empty());
        let lines = b.push(b"ld\n");
        assert_eq!(lines, vec!["hello world".to_string()]);
    }

    #[test]
    fn multiple_lines_in_one_push() {
        let mut b = LineBuffer::new();
        let lines = b.push(b"a\nb\nc\n");
        assert_eq!(lines, vec!["a", "b", "c"]);
    }

    #[test]
    fn crlf_is_stripped() {
        let mut b = LineBuffer::new();
        let lines = b.push(b"data: foo\r\n");
        assert_eq!(lines, vec!["data: foo".to_string()]);
    }

    #[test]
    fn utf8_word_split_across_chunks_survives() {
        // "à rebours" — accent is two bytes (0xC3 0xA0); the cut is between
        // valid char boundaries here, between "à reb" and "ours\n".
        let mut b = LineBuffer::new();
        assert!(b.push("à reb".as_bytes()).is_empty());
        let lines = b.push(b"ours\n");
        assert_eq!(lines, vec!["à rebours".to_string()]);
    }

    #[test]
    fn trailing_partial_stays_until_completed() {
        let mut b = LineBuffer::new();
        assert!(b.push(b"partial").is_empty());
        assert_eq!(b.take_remaining(), Some("partial".to_string()));
        assert!(b.take_remaining().is_none());
    }
}
