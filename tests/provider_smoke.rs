//! Credential-gated provider smoke tests — the evidence path for promoting a
//! provider route from 🔶 Alpha to ✅ Stable.
//!
//! Each test does a tiny real completion against a live provider and asserts a
//! token streams back. **If the provider's API key env var is absent, the test
//! SKIPS (and passes)** — so the suite is a no-op offline and in CI without
//! secrets, but becomes real proof the moment a key is present (locally or in a
//! secrets-enabled CI job).
//!
//! Run a single provider locally, e.g.:
//!   ANTHROPIC_API_KEY=sk-... cargo test --test provider_smoke -- --nocapture anthropic
//!
//! Optional model overrides (defaults are cheap models):
//!   SPARROW_SMOKE_ANTHROPIC_MODEL, SPARROW_SMOKE_OPENAI_MODEL, SPARROW_SMOKE_GROQ_MODEL

use std::time::Duration;

use futures::StreamExt;
use sparrow::provider::{Brain, BrainEvent, BrainRequest, ContentBlock, Msg, PromptCacheConfig};

fn tiny_request() -> BrainRequest {
    BrainRequest {
        system: Some("You are a test probe. Reply with exactly one word.".into()),
        messages: vec![Msg {
            role: "user".into(),
            content: vec![ContentBlock::Text {
                text: "Say the single word: pong".into(),
            }],
        }],
        tools: vec![],
        max_tokens: 16,
        temperature: 0.0,
        stop: vec![],
        cache: PromptCacheConfig::disabled(),
    }
}

/// Drive a brain through one completion and assert at least one non-empty text
/// token arrives within a timeout. Shared by every provider test.
async fn assert_streams_text(provider: &str, brain: impl Brain) {
    let stream = tokio::time::timeout(Duration::from_secs(45), brain.complete(tiny_request()))
        .await
        .unwrap_or_else(|_| panic!("{provider}: complete() timed out"))
        .unwrap_or_else(|e| panic!("{provider}: complete() failed: {e}"));

    let mut stream = stream;
    let mut got_text = false;
    let consume = async {
        while let Some(ev) = stream.next().await {
            if let BrainEvent::TextDelta(t) = ev
                && !t.trim().is_empty()
            {
                got_text = true;
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(45), consume)
        .await
        .unwrap_or_else(|_| panic!("{provider}: stream did not complete in time"));

    assert!(
        got_text,
        "{provider}: expected at least one non-empty text token"
    );
}

fn skip(provider: &str, env_var: &str) {
    eprintln!("[skip] {provider}: {env_var} not set — set it to run this smoke test");
}

#[tokio::test]
async fn anthropic_streams_text_when_keyed() {
    let Ok(key) = std::env::var("ANTHROPIC_API_KEY") else {
        return skip("anthropic", "ANTHROPIC_API_KEY");
    };
    let model = std::env::var("SPARROW_SMOKE_ANTHROPIC_MODEL")
        .unwrap_or_else(|_| "claude-haiku-4-5-20251001".into());
    let brain = sparrow::provider::anthropic::AnthropicAdapter::new(&model, key, None);
    assert_streams_text("anthropic", brain).await;
}

#[tokio::test]
async fn openai_streams_text_when_keyed() {
    let Ok(key) = std::env::var("OPENAI_API_KEY") else {
        return skip("openai", "OPENAI_API_KEY");
    };
    let model =
        std::env::var("SPARROW_SMOKE_OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
    let brain = sparrow::provider::openai_compat::OpenAICompatAdapter::new(
        &model,
        key,
        "https://api.openai.com/v1",
    );
    assert_streams_text("openai", brain).await;
}

#[tokio::test]
async fn groq_streams_text_when_keyed() {
    let Ok(key) = std::env::var("GROQ_API_KEY") else {
        return skip("groq", "GROQ_API_KEY");
    };
    let model =
        std::env::var("SPARROW_SMOKE_GROQ_MODEL").unwrap_or_else(|_| "llama-3.1-8b-instant".into());
    let brain = sparrow::provider::openai_compat::OpenAICompatAdapter::new(
        &model,
        key,
        "https://api.groq.com/openai/v1",
    );
    assert_streams_text("groq", brain).await;
}
