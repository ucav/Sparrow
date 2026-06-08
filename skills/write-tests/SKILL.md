# Skill: Write Tests

**Trigger:** tests, test coverage, écris des tests, add tests

**Description:** Génération de tests unitaires et d'intégration. Arrange-Act-Assert. Couvre cas nominaux, edge cases, erreurs. Property testing avec proptest.

## Body

### Structure AAA (Arrange-Act-Assert)
```rust
#[test]
fn test_user_creation_valid_email() {
    // Arrange
    let email = "test@example.com";
    // Act
    let user = User::new(email);
    // Assert
    assert!(user.is_ok());
    assert_eq!(user.unwrap().email, email);
}
```

### Cas à couvrir
1. **Happy path** — entrée normale, résultat attendu
2. **Edge cases** — chaîne vide, 0, -1, MAX_VALUE, None
3. **Erreurs** — entrée invalide, état impossible
4. **Concurrence** — si async, tester avec `tokio::test`

### Commandes
```bash
cargo test                          # Tous les tests
cargo test test_user                # Filtre par nom
cargo test -- --nocapture           # Voir les println!
cargo test -- --test-threads=1      # Séquentiel
cargo tarpaulin                     # Couverture de code
```

### Property testing (proptest)
```rust
proptest! {
    #[test]
    fn test_any_email_is_valid(s in "\\w+@\\w+\\.\\w+") {
        assert!(User::new(&s).is_ok());
    }
}
```

### Pièges
- Ne pas tester l'implémentation, tester le comportement
- Un test par concept, pas un test par ligne de code
- Les tests doivent être rapides (<100ms idéalement)
