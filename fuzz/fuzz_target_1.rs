// ─── Fuzz targets for Sparrow (Phase 11 Item 34) ──────────────────────────────
// Run with: cargo fuzz run fuzz_config_roundtrip

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz config deserialization
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = toml::from_str::<sparrow::config::Config>(s);
    }
});
