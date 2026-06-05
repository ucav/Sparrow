#[test]
fn docs_site_exposes_search_examples_api_and_video_tutorials() {
    let html = std::fs::read_to_string("docs/index.html").expect("docs/index.html must exist");

    for marker in [
        "id=\"docs\"",
        "id=\"docsSearch\"",
        "DOCS_INDEX",
        "renderDocsSearch",
        "data-testid=\"docs-search-results\"",
        "id=\"examples\"",
        "data-copy=\"sparrow setup",
        "id=\"api\"",
        "CLI API",
        "Tool API",
        "WebView API",
        "id=\"videos\"",
        "<video controls",
        "tutorials/first-launch.mp4",
        "tutorials/safe-edits-rewind.mp4",
        "tutorials/memory-graph-browser.mp4",
        "tutorials/README.md#first-launch-in-5-minutes",
        "Video tutorial: first launch",
        "Video tutorial: safe agent edits",
        "Video tutorial: memory graph and browser",
    ] {
        assert!(
            html.contains(marker),
            "docs/index.html must expose docs site marker `{marker}`"
        );
    }

    assert!(
        !html.contains("Video placeholder"),
        "docs/index.html must ship real video tutorials, not placeholder cards"
    );

    for video in [
        "docs/tutorials/first-launch.mp4",
        "docs/tutorials/safe-edits-rewind.mp4",
        "docs/tutorials/memory-graph-browser.mp4",
    ] {
        let metadata = std::fs::metadata(video).unwrap_or_else(|err| {
            panic!("tutorial video `{video}` must exist and be readable: {err}")
        });
        assert!(
            metadata.len() > 64_000,
            "tutorial video `{video}` should be a real generated MP4, not an empty placeholder"
        );
    }

    let transcript = std::fs::read_to_string("docs/tutorials/README.md")
        .expect("tutorial transcript must exist");
    for marker in [
        "First Launch In 5 Minutes",
        "Safe Edits And Rewind",
        "Memory Graph + Browser-Use",
        "sparrow setup",
        "sparrow rewind <checkpoint-id>",
        "npm run browser:install",
    ] {
        assert!(
            transcript.contains(marker),
            "tutorial transcript must include `{marker}`"
        );
    }
}
