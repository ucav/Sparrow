# Skill: Write Unit Tests

**Trigger:** écris des tests, generate tests, test this function, add test coverage

**Description:** Génère des tests unitaires complets pour une fonction ou un module donné. Couvre les cas nominaux, les edge cases, et les erreurs.

## Body

1. **Comprendre** : ils la fonction cible, ses paramètres, son type de retour
2. **Cas nominaux** : test avec des entrées "normales"
3. **Edge cases** : valeurs limites, chaînes vides, `None`, zéro, nombres négatifs
4. **Erreurs** : entrées invalides, états impossibles
5. **Style** : suis les conventions du projet (pytest pour Python, `#[test]` pour Rust, etc.)
6. **Nommage** : `test_<fonction>_<scénario>_<résultat_attendu>`

Pour chaque test :
- Arrange : prépare les données
- Act : appelle la fonction
- Assert : vérifie le résultat

Exemple Rust :
```rust
#[test]
fn test_parse_config_valid_toml() {
    let input = r#"[server]\nhost = \"localhost\"\nport = 8080"#;
    let config = parse_config(input).unwrap();
    assert_eq!(config.server.host, "localhost");
}

#[test]
fn test_parse_config_empty_input() {
    let result = parse_config("");
    assert!(result.is_err());
}
```
