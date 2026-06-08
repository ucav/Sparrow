# Skill: Prompt Engineering

**Trigger:** prompt, LLM prompt, AI prompt, system prompt

**Description:** Patterns de prompt engineering : system prompts, few-shot, chain-of-thought, contraintes de sortie, anti-patterns.

## Body

### Structure d'un bon prompt
```
1. RÔLE : "Tu es un expert Rust avec 10 ans d'expérience."
2. TÂCHE : "Explique le concept de borrowing."
3. FORMAT : "Réponds en 3 paragraphes avec des exemples de code."
4. CONTRAINTES : "Pas de jargon inutile. Code compilable."
5. EXEMPLES : few-shot si nécessaire
```

### Patterns éprouvés
```
# Chain-of-Thought
"Résous ce problème étape par étape. Explique ton raisonnement avant de donner la réponse."

# Few-shot
"Exemple 1: [entrée] → [sortie]. Exemple 2: [entrée] → [sortie]. Maintenant: [nouvelle entrée]"

# Persona
"Tu es un code reviewer senior. Tu es direct, constructif, et tu cites des lignes de code spécifiques."

# Structured output
"Réponds UNIQUEMENT en JSON: {\"analysis\": \"...\", \"fix\": \"...\", \"confidence\": 0-10}"
```

### Anti-patterns (ce qu'il faut éviter)
```
❌ "Fais de ton mieux"          → trop vague
❌ "Sois créatif"               → pas de contrainte
❌ "Explique-moi tout sur X"    → scope infini
❌ "Réponds oui ou non"         → trop restrictif, pas de contexte
❌ Prompts de 500+ mots         → dilution du signal
```

### Tokens & Coûts
- 1 token ≈ 0.75 mot anglais ≈ 4 caractères
- System prompt trop long → bouffe le context window
- Toujours demander le format de sortie exact pour éviter du blabla inutile
