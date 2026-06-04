# Sparrow Fix Plan — Streaming Corruption

## Diagnostiqué
- **Fichier**: `src/provider/openai_compat.rs` L214-362
- **Cause**: parseur SSE sans buffer inter-chunk
- **Symptômes**: texte tronqué mi-mot, tool cards manquantes

## Fix

### Fix 1: SSE buffer (openai_compat.rs)
- Remplacer `scan` sur `bytes_stream()` par un `fold` avec buffer d'octets
- Accumuler les bytes dans un `Vec<u8>` entre chunks
- Splitter sur `\n\n` (séparateur SSE standard)
- Parser chaque event SSE complet (lignes `data:` suivies de `\n\n`)
- Garder le reste non-complet pour le prochain chunk

### Fix 2: model_override cleanup (console.rs)
- Vérifier qu'on strip le prefix provider du model_override
- Par ex `deepseek-v4-pro` → envoyer juste le model name, pas `provider:model`

### Fix 3: DeepSeek reasoning_content (déjà fait L246-252)
- Vérifié, déjà implémenté. OK.

## Ordre
1. Fix 2 (console.rs) — plus simple
2. Fix 1 (openai_compat.rs) — le vrai fix
3. Build PS → kill → restart
4. Smoke test: vérifier texte complet + tool cards visibles
