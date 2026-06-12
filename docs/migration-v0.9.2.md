# Migration Notes: Sparrow v0.9.2

No breaking config migration is expected for v0.9.2.

- Existing `config.toml` files continue to load.
- Existing transcripts remain additive-serde compatible.
- `sparrow console` keeps normal browser auto-open behavior.
- `sparrow console --fast` is new and intentionally does not auto-open a browser.
- `intel.enabled` defaults to `false`; no public release scan runs unless
  explicitly enabled or invoked with `sparrow intel scan --source/--config`.
- Optional skill manifests (`manifest.toml` / `manifest.json`) are additive.
  Existing `SKILL.md` files keep working without a manifest.
- `/tools` now includes `manifests` in addition to legacy metadata; existing
  WebView consumers can keep reading the old `tools` field.
- WebView premium panels read local endpoints only: `/intel/backlog`,
  `/intel/digests`, and `/runs`.
