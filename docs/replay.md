# Replay & Reproducibility

## Transcript Format

Every run produces an append-only transcript:

```
~/.local/state/sparrow/transcripts/<run-id>/
├── inputs.json      # task, config snapshot, model, repo HEAD, timestamp
├── events.jsonl     # one Event per line (NDJSON)
└── meta.json        # run_id, event count, created_at
```

## Replay

```bash
sparrow replay <run-id>     # re-render the transcript
sparrow replay <run-id>     # (prompts to re-execute against current model)
```

The replay scrubber in the TUI lets you scrub through the timeline.

## Audit Trail

- Every approval decision is an event
- Every tool call is an event
- Transcripts are shareable and diffable
- `inputs.json` captures the exact config and model state

## Re-execute

A transcript can be re-executed against a different model for comparison. Same inputs, different brain → compare outcomes.
