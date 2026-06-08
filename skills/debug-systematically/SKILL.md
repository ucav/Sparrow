# Skill: Debug Systematically

**Trigger:** debug, bug, error, crash, ne marche pas, investigate

**Description:** Méthode systématique de debugging en 5 phases. Reproduire → Isoler → Comprendre → Corriger → Vérifier. Évite le debugging aléatoire par print.

## Body

### Phase 1 : Reproduire
```bash
# Isoler le bug dans un test minimal
git stash && git checkout -b debug/issue-123
# Écrire un test qui échoue (RED)
cargo test test_bug_123 -- --nocapture
```

### Phase 2 : Isoler
```bash
# Binary search dans l'historique si régression
git bisect start
git bisect bad HEAD
git bisect good v0.5.0
# À chaque étape : cargo test, puis git bisect good/bad

# Ou réduire le problème : commenter du code jusqu'à trouver le minimum
```

### Phase 3 : Comprendre
```bash
# Logs détaillés
RUST_LOG=debug cargo run 2>&1 | tee debug.log

# Backtrace complète
RUST_BACKTRACE=full cargo run

# Inspecter l'état
cargo run -- --json run "debug this" | jq '.events[] | select(.type=="ToolUseProposed")'
```

### Phase 4 : Corriger
- UNE chose à la fois
- Le test doit passer (GREEN)
- Pas de régression : `cargo test`

### Phase 5 : Vérifier
```bash
cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

### Pièges courants
- Ne pas corriger le symptôme — trouver la cause racine
- Ne pas ajouter de `println!` partout — utiliser `dbg!()` ou un logger
- Vérifier que le fix ne crée pas 2 nouveaux bugs
