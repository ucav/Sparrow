//! Voice command for Sparrow.
//!
//! Integrates TTS and STT tools into a unified voice interface.

use std::path::PathBuf;

/// Voice operation mode.
pub enum VoiceCommand {
    /// Convert text to speech
    Speak {
        text: String,
        output: Option<PathBuf>,
    },
    /// Transcribe audio to text
    Transcribe {
        audio_file: PathBuf,
        language: Option<String>,
    },
    /// List available voices/providers
    ListProviders,
}

/// Handle a voice command.
pub fn handle_voice(cmd: VoiceCommand) -> anyhow::Result<()> {
    match cmd {
        VoiceCommand::Speak { text, output } => {
            println!("🔊 Converting text to speech...");
            let path = crate::tools::tts::text_to_speech(
                &text,
                crate::tools::tts::TtsProvider::Edge,
                output,
            )?;
            println!("✓ Audio saved: {}", path.display());
            println!("  → Play: ffplay {} -nodisp -autoexit", path.display());
        }
        VoiceCommand::Transcribe { audio_file, language } => {
            println!("🎤 Transcribing audio...");
            let text = crate::tools::stt::transcribe(
                &audio_file,
                language.as_deref(),
                crate::tools::stt::SttBackend::OpenAIApi,
            )?;
            println!("✓ Transcription:\n\n{text}");
        }
        VoiceCommand::ListProviders => {
            println!("Available TTS providers:");
            println!("  edge      — Microsoft Edge TTS (free, built-in voices)");
            println!("  openai    — OpenAI TTS API (requires API key)");
            println!("  say       — macOS built-in (free)");
            println!("  espeak    — Linux espeak (free, install: apt install espeak)");
            println!();
            println!("Available STT providers:");
            println!("  openai    — OpenAI Whisper API (requires API key)");
            println!("  whisper   — Local whisper CLI (install: pip install openai-whisper)");
            println!("  whisper-cpp — Local whisper.cpp (fast, no Python needed)");
        }
    }
    Ok(())
}
