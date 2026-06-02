#![cfg(feature = "treesitter")]

use sparrow::memory::symbol_index::{SymbolIndex, SymbolKind};

fn temp_ws(name: &str) -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir().join(format!("sparrow-{name}-{id}"));
    std::fs::create_dir_all(d.join("src")).unwrap();
    d
}

#[test]
fn tree_sitter_symbol_index_finds_rust_definitions() {
    let ws = temp_ws("symbol-index");
    let source = r#"pub fn foo() -> u8 {
    1
}

pub struct Bar {
    value: u8,
}
"#;
    std::fs::write(ws.join("src").join("lib.rs"), source).unwrap();

    let index = SymbolIndex::build(&ws);
    let foo = index.find_definition("foo");
    assert_eq!(foo.len(), 1, "expected one foo definition, got {foo:?}");
    assert_eq!(foo[0].file, std::path::PathBuf::from("src").join("lib.rs"));
    assert_eq!(foo[0].line, 1);
    assert_eq!(foo[0].kind, SymbolKind::Fn);

    let bar = index.find_definition("Bar");
    assert_eq!(bar.len(), 1, "expected one Bar definition, got {bar:?}");
    assert_eq!(bar[0].file, std::path::PathBuf::from("src").join("lib.rs"));
    assert_eq!(bar[0].line, 5);
    assert_eq!(bar[0].kind, SymbolKind::Struct);

    let _ = std::fs::remove_dir_all(&ws);
}
