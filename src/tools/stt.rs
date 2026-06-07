//! Speech-to-Text tool for Sparrow.
//!
//! Transcribes audio files to text using Whisper (local or API).

use std::path::Path;
use std::process::Command;

/// Available STT backends.
#[derive(Debug, Clone)]
pub enum SttBackend {
    /// OpenAI Whisper API
    OpenAIApi,
    /// Local Whisper CLI (whisper-rs or openai-whisper)
    WhisperCli,
    /// Local whisper.cpp
    WhisperCpp,
}

/// Transcribe an audio file to text.
///
/// Returns the transcription text.
pub fn transcribe(
    audio_path: &Path,
    language: Option<&str>,
    backend: SttBackend,
) -> anyhow::Result<String> {
    if !audio_path.exists() {
        anyhow::bail!("Audio file not found: {}", audio_path.display());
    }

    match backend {
        SttBackend::OpenAIApi => transcribe_openai(audio_path, language),
        SttBackend::WhisperCli => transcribe_whisper_cli(audio_path, language),
        SttBackend::WhisperCpp => transcribe_whisper_cpp(audio_path, language),
    }
}

/// Use OpenAI Whisper API.
fn transcribe_openai(audio_path: &Path, language: Option<&str>) -> anyhow::Result<String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY environment variable not set"))?;

    let client = reqwest::blocking::Client::new();
    let form = reqwest::blocking::multipart::Form::new()
        .file("file", audio_path)?
        .text("model", "whisper-1");

    let form = if let Some(lang) = language {
        form.text("language", lang.to_string())
    } else {
        form
    };

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()?;

    if !resp.status().is_success() {
        let body = resp.text()?;
        anyhow::bail!("OpenAI Whisper failed: {body}");
    }

    let json: serde_json::Value = resp.json()?;
    json.get("text")
        .and_then(|t| t.as_str())
        .map(|t| t.to_string())
        .ok_or_else(|| anyhow::anyhow!("No transcription in response"))
}

/// Use local whisper CLI.
fn transcribe_whisper_cli(audio_path: &Path, language: Option<&str>) -> anyhow::Result<String> {
    let mut cmd = Command::new("whisper");
    cmd.arg(audio_path);

    if let Some(lang) = language {
        cmd.args(["--language", lang]);
    }

    cmd.arg("--output_format").arg("txt");
    cmd.arg("--output_dir")
        .arg(audio_path.parent().unwrap_or(Path::new(".")));

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("whisper CLI failed: {stderr}\nInstall: pip install openai-whisper");
    }

    // Read the generated .txt file
    let txt_path = audio_path.with_extension("txt");
    if txt_path.exists() {
        Ok(std::fs::read_to_string(&txt_path)?)
    } else {
        // Output is in stdout
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Use local whisper.cpp.
fn transcribe_whisper_cpp(audio_path: &Path, language: Option<&str>) -> anyhow::Result<String> {
    let mut cmd = Command::new("whisper-cpp");
    cmd.arg("-f").arg(audio_path);

    if let Some(lang) = language {
        cmd.arg("-l").arg(lang);
    }

    cmd.arg("-otxt");

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("whisper-cpp failed: {stderr}");
    }

    // Read the generated .txt file
    let txt_path = audio_path.with_extension("txt");
    if txt_path.exists() {
        Ok(std::fs::read_to_string(&txt_path)?)
    } else {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
