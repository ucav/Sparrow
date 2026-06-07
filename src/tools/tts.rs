//! Text-to-Speech tool for Sparrow.
//!
//! Converts text to speech audio using various providers (edge-tts by default).
//! Saves audio files in the Sparrow state directory.

use std::path::PathBuf;
use std::process::Command;

/// Available TTS providers.
#[derive(Debug, Clone)]
pub enum TtsProvider {
    /// Microsoft Edge TTS (free, built-in voices)
    Edge,
    /// OpenAI TTS API
    OpenAI,
    /// System `say` command (macOS)
    Say,
    /// System `espeak` command (Linux)
    Espeak,
}

/// Convert text to speech and save to a file.
///
/// Returns the path to the generated audio file.
pub fn text_to_speech(
    text: &str,
    provider: TtsProvider,
    output_dir: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let dir = output_dir.unwrap_or_else(|| {
        let mut d = dirs::state_dir().unwrap_or_else(|| PathBuf::from("."));
        d.push("sparrow");
        d.push("audio");
        d
    });
    std::fs::create_dir_all(&dir)?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_path = dir.join(format!("tts_{timestamp}.mp3"));

    match provider {
        TtsProvider::Edge => tts_edge(text, &output_path)?,
        TtsProvider::OpenAI => tts_openai(text, &output_path)?,
        TtsProvider::Say => tts_say(text, &output_path)?,
        TtsProvider::Espeak => tts_espeak(text, &output_path)?,
    }

    Ok(output_path)
}

/// Use Microsoft Edge TTS (free, no API key needed).
fn tts_edge(text: &str, output: &std::path::Path) -> anyhow::Result<()> {
    // edge-tts is a Python package: pip install edge-tts
    let status = Command::new("edge-tts")
        .args([
            "--text",
            text,
            "--voice",
            "fr-FR-DeniseNeural",
            "--write-media",
            &output.to_string_lossy(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => {
            // Try English fallback
            Command::new("edge-tts")
                .args([
                    "--text",
                    text,
                    "--voice",
                    "en-US-JennyNeural",
                    "--write-media",
                    &output.to_string_lossy(),
                ])
                .status()?;
            Ok(())
        }
        Err(_) => {
            anyhow::bail!(
                "edge-tts not found. Install it with: pip install edge-tts\n\
                 Or use another provider: text_to_speech(text, provider=TtsProvider::Espeak)"
            );
        }
    }
}

/// Use OpenAI TTS API.
fn tts_openai(text: &str, output: &std::path::Path) -> anyhow::Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .or_else(|_| std::env::var("OPENAI_TTS_KEY"))
        .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY environment variable not set"))?;

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post("https://api.openai.com/v1/audio/speech")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": "tts-1",
            "input": text,
            "voice": "alloy",
            "response_format": "mp3",
        }))
        .send()?;

    if !resp.status().is_success() {
        let body = resp.text()?;
        anyhow::bail!("OpenAI TTS failed: {body}");
    }

    let bytes = resp.bytes()?;
    std::fs::write(output, bytes)?;
    Ok(())
}

/// Use macOS `say` command.
fn tts_say(text: &str, output: &std::path::Path) -> anyhow::Result<()> {
    let status = Command::new("say")
        .args(["-o", &output.with_extension("aiff").to_string_lossy(), text])
        .status()?;

    if !status.success() {
        anyhow::bail!("say command failed");
    }
    Ok(())
}

/// Use Linux `espeak` command.
fn tts_espeak(text: &str, output: &std::path::Path) -> anyhow::Result<()> {
    let wav_path = output.with_extension("wav");
    let status = Command::new("espeak")
        .args(["-w", &wav_path.to_string_lossy(), text])
        .status()?;

    if !status.success() {
        anyhow::bail!("espeak command failed. Install: sudo apt install espeak");
    }
    Ok(())
}
