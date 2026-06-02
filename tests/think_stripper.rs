// Verifies the streaming <think> stripper: hides reasoning blocks, passes
// normal text through, handles tags split across deltas, and FAILS OPEN
// (recovers an unclosed think block on flush so answers are never swallowed).

use sparrow::event::ThinkStripper;

#[test]
fn passes_normal_text_through() {
    let mut s = ThinkStripper::new();
    let mut out = String::new();
    out.push_str(&s.feed("Rust is "));
    out.push_str(&s.feed("a systems language."));
    out.push_str(&s.flush());
    assert_eq!(out, "Rust is a systems language.");
}

#[test]
fn hides_closed_think_block() {
    let mut s = ThinkStripper::new();
    let mut out = String::new();
    out.push_str(&s.feed("<think>let me reason about this</think>The answer is 42."));
    out.push_str(&s.flush());
    assert_eq!(out, "The answer is 42.");
}

#[test]
fn handles_tag_split_across_deltas() {
    let mut s = ThinkStripper::new();
    let mut out = String::new();
    // "<think>" split across three deltas
    out.push_str(&s.feed("before <th"));
    out.push_str(&s.feed("ink>secret reason"));
    out.push_str(&s.feed("ing</thi"));
    out.push_str(&s.feed("nk>after"));
    out.push_str(&s.flush());
    assert_eq!(out, "before after");
}

#[test]
fn fails_open_on_unclosed_think() {
    // A model that opens <think> but never closes it must NOT swallow the answer.
    let mut s = ThinkStripper::new();
    let mut out = String::new();
    out.push_str(&s.feed("<think>reasoning then the actual answer here"));
    // stream ends without </think>
    out.push_str(&s.flush());
    assert!(
        out.contains("actual answer here"),
        "unclosed think must be recovered on flush, got: {out:?}"
    );
}
