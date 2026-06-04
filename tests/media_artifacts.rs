use sparrow::console::{MAX_ATTACHMENT_BYTES, classify_attachment};
use sparrow::tools::extras::{ImageGeneration, TextToSpeech};
use sparrow::tools::media::{ImageGen, Transcribe, Tts};
use sparrow::tools::{Tool, ToolCtx, known_tool_metadata};

fn ctx() -> ToolCtx {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path().to_path_buf();
    // Leak the tempdir so the path stays valid for the test duration.
    std::mem::forget(tmp);
    ToolCtx {
        workspace_root: root,
        run_id: sparrow::event::RunId("test-run".into()),
    }
}

fn unset(name: &str) {
    // Tests run sequentially in one process; clear keys to force "missing key"
    // behavior even when the developer has them in their shell.
    // SAFETY: std::env::remove_var is `unsafe` in 2024 edition; we accept that.
    unsafe {
        std::env::remove_var(name);
    }
}

#[tokio::test]
async fn image_gen_without_key_is_clear_error() {
    unset("IMAGE_API_KEY");
    unset("OPENAI_API_KEY");
    let tool = ImageGen::new();
    let ctx = ctx();
    let res = tool
        .call(serde_json::json!({"prompt": "hello"}), &ctx)
        .await
        .expect("call");
    // ToolResult::error sets is_error=true; check by looking at the text Block.
    let text = format!("{:?}", res);
    assert!(
        text.contains("No image API key"),
        "expected honest error, got {:?}",
        text
    );
}

#[tokio::test]
async fn tts_without_key_is_clear_error() {
    unset("TTS_API_KEY");
    unset("OPENAI_API_KEY");
    let tool = Tts::new();
    let res = tool
        .call(serde_json::json!({"text": "hi"}), &ctx())
        .await
        .expect("call");
    let text = format!("{:?}", res);
    assert!(text.contains("No TTS API key"), "got {:?}", text);
}

#[tokio::test]
async fn legacy_media_aliases_delegate_to_real_tools() {
    unset("IMAGE_API_KEY");
    unset("TTS_API_KEY");
    unset("OPENAI_API_KEY");

    let image = ImageGeneration
        .call(serde_json::json!({"prompt": "sparrow"}), &ctx())
        .await
        .expect("image alias");
    assert!(
        format!("{:?}", image).contains("No image API key"),
        "image alias should surface the real image tool error, got {:?}",
        image
    );

    let tts = TextToSpeech
        .call(serde_json::json!({"text": "hello"}), &ctx())
        .await
        .expect("tts alias");
    assert!(
        format!("{:?}", tts).contains("No TTS API key"),
        "tts alias should surface the real TTS tool error, got {:?}",
        tts
    );
}

#[tokio::test]
async fn transcribe_without_key_is_clear_error() {
    unset("TRANSCRIBE_API_KEY");
    unset("OPENAI_API_KEY");
    let tool = Transcribe::new();
    let res = tool
        .call(serde_json::json!({"path": "voice.mp3"}), &ctx())
        .await
        .expect("call");
    let text = format!("{:?}", res);
    assert!(text.contains("No transcription API key"), "got {:?}", text);
}

#[test]
fn classify_attachment_handles_known_types() {
    assert_eq!(classify_attachment("image/png", "png"), "image");
    assert_eq!(classify_attachment("audio/mpeg", "mp3"), "audio");
    assert_eq!(classify_attachment("application/pdf", "pdf"), "pdf");
    assert_eq!(classify_attachment("text/plain", "txt"), "text");
    assert_eq!(
        classify_attachment("application/octet-stream", "bin"),
        "file"
    );
    // Mime-only classification (no extension) still works.
    assert_eq!(classify_attachment("image/jpeg", ""), "image");
    // Extension-only fallback works (when mime is generic).
    assert_eq!(
        classify_attachment("application/octet-stream", "wav"),
        "audio"
    );
}

#[test]
fn attachment_size_limit_is_10mb() {
    // Just an explicit sanity check so the value isn't silently changed.
    assert_eq!(MAX_ATTACHMENT_BYTES, 10 * 1024 * 1024);
}

#[test]
fn known_tool_metadata_lists_real_media_tools() {
    let names: Vec<_> = known_tool_metadata(None)
        .into_iter()
        .map(|tool| tool.name)
        .collect();
    assert!(names.contains(&"image_generate".to_string()));
    assert!(names.contains(&"text_to_speech".to_string()));
    assert!(names.contains(&"transcribe".to_string()));
    assert!(!names.contains(&"image_gen".to_string()));
    assert!(!names.contains(&"tts".to_string()));
}
