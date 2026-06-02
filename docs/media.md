# Media, voice & artifacts

Sparrow exposes four multimodal building blocks plus a small WebView surface
for attachments and artifacts. All cloud calls hit OpenAI-compatible endpoints
so any provider that mirrors them works. **Missing key or non-2xx response is
always a real, surfaced error — never a fake success.**

## Tools

### `vision`

Loads an image from the workspace, base64-encodes it, and returns an
`Image` block alongside a `Text` block so a vision-capable model can answer a
question about it.

### `image_generate`

Posts a prompt to `<IMAGE_API_BASE>/images/generations` (default
`https://api.openai.com/v1`, default model `gpt-image-1`). The result is
written into the workspace as a PNG and the path is returned.

Env: `IMAGE_API_KEY` (or `OPENAI_API_KEY`), `IMAGE_API_BASE`, `IMAGE_MODEL`.

### `text_to_speech`

Posts text to `<TTS_API_BASE>/audio/speech` (default OpenAI, default model
`gpt-4o-mini-tts`). The audio bytes are written into the workspace.

Env: `TTS_API_KEY` (or `OPENAI_API_KEY`), `TTS_API_BASE`, `TTS_MODEL`.

### `transcribe`

Posts a workspace audio file to `<TRANSCRIBE_API_BASE>/audio/transcriptions`
(default OpenAI, default model `whisper-1`) as `multipart/form-data` and
returns the transcript as a `Text` block. Optional `language` arg for an
ISO-639-1 hint.

Env: `TRANSCRIBE_API_KEY` (or `OPENAI_API_KEY`), `TRANSCRIBE_API_BASE`,
`TRANSCRIBE_MODEL`.

This is the building block for minimal voice mode: import an audio file via
the WebView attachments endpoint, then call `transcribe` on it, then drive a
normal text run.

## WebView attachments

`POST /upload` accepts `multipart/form-data` with any number of file fields.

- Each file is capped at **10 MB** (`MAX_ATTACHMENT_BYTES`).
- Files are written to `.sparrow/attachments/` in the current working
  directory. Path traversal is stripped (`Path::file_name`).
- Each accepted file gets metadata: `name`, `path`, `size`, `mime`, `kind`
  (one of `image`, `audio`, `pdf`, `text`, `file`).
- Files exceeding the cap or failing to write are reported in `rejected`
  with a clear reason — uploads never silently fail.

`GET /artifacts` lists everything in `.sparrow/attachments/` with the same
metadata shape so the WebView can render inline images, an audio player,
PDF metadata, or a plain download link based on `kind`.

## Status

**Alpha.** The tools exist and surface honest errors. Real-microphone capture
is intentionally out of scope on the CLI side; voice mode is "import audio
file → transcribe → reply."
