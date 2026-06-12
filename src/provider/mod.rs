//! Provider layer. The Brain trait and the model adapters (Anthropic,
//! OpenAI-compatible, Ollama, Responses) live in the `sparrow-providers` crate
//! so the heavy adapter code compiles once and is cached — touching the engine
//! no longer recompiles them. Re-exported here so existing `crate::provider::*`
//! imports keep working unchanged.
//!
//! `detect` (first-run provider detection) stays in the binary crate because it
//! depends on the config provider registry and the onboarding wizard.

pub use sparrow_providers::*;

pub mod detect;
