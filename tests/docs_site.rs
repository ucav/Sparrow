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
        "Video tutorial: first launch",
        "Video tutorial: safe agent edits",
        "Video tutorial: memory graph and browser",
    ] {
        assert!(
            html.contains(marker),
            "docs/index.html must expose docs site marker `{marker}`"
        );
    }
}
