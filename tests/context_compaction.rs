use sparrow::context::{ContextMeter, HandoffDoc, distill_transcript};
use sparrow::hooks::HookEvent;

#[test]
fn meter_summary_includes_all_categories() {
    let mut m = ContextMeter::new(8000);
    m.prompt_chars = 1200;
    m.memory_chars = 200;
    m.tools_chars = 400;
    m.attachments_chars = 0;
    m.transcript_chars = 5000;
    let s = m.summary();
    assert!(s.contains("prompt 1200"));
    assert!(s.contains("memory 200"));
    assert!(s.contains("tools 400"));
    assert!(s.contains("attach 0"));
    assert!(s.contains("transcript 5000"));
    assert!(s.contains("8000t"));
}

#[test]
fn compaction_preserves_last_decisions_in_handoff() {
    let messages = vec![
        "Read src/main.rs and src/lib.rs.".into(),
        "Decision: split the engine into modules".into(),
        "Decision: keep streaming as-is".into(),
        "cargo test --all-targets passed".into(),
    ];
    let doc = distill_transcript(&messages);
    let last = doc.decisions.last().expect("at least one decision");
    assert!(
        last.to_lowercase().contains("streaming"),
        "last decision must survive distillation, got {:?}",
        doc.decisions
    );
    assert!(
        doc.tests_run.iter().any(|t| t.contains("cargo test")),
        "tests must be preserved: {:?}",
        doc.tests_run
    );
}

#[test]
fn handoff_doc_writes_well_formed_markdown_to_disk() {
    let tmp = tempfile::tempdir().expect("tmp");
    let path = tmp.path().join("handoff.md");
    let mut doc = HandoffDoc::new("phase 12 smoke");
    doc.files_modified = vec!["src/context.rs".into()];
    doc.decisions = vec!["decision: ship context meter".into()];
    doc.next_steps = vec!["wire pre-compact hook".into()];
    std::fs::write(&path, doc.to_markdown()).expect("write");
    let read_back = std::fs::read_to_string(&path).expect("read");
    assert!(read_back.contains("# Sparrow Handoff"));
    assert!(read_back.contains("src/context.rs"));
    assert!(read_back.contains("ship context meter"));
    assert!(read_back.contains("wire pre-compact hook"));
}

#[test]
fn pre_compact_and_post_compact_hook_events_exist() {
    // Phase 12 added these variants; this test pins them so removing them is
    // a breaking change picked up by CI.
    let pre = HookEvent::PreCompact;
    let post = HookEvent::PostCompact;
    assert_ne!(format!("{:?}", pre), format!("{:?}", post));
}
