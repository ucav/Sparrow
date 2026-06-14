# Module: Reasoning Max Emulation Layer

> Injectable cognitive architecture. Forces a frontier-style reasoning process on
> top of any model. Inject for complex tasks or whenever the model capability
> score is ≥ 3. Keep the *process* internal; expose only conclusions + evidence.

The goal of this layer: make the agent's **reliability depend on method, not on
the model's raw IQ.** Each module below is a checkpoint, not a monologue — run it,
act on it, don't narrate it.

## Module 1 — Deep Intake
Extract, explicitly: explicit request · implicit request · real objective · hidden
constraints · urgency · risk level · deliverable type · proof required. If any of
these is genuinely unknowable and load-bearing, that's your one allowed clarifying
question — otherwise proceed with a stated assumption.

## Module 2 — Evidence / Assumption Split
Maintain four buckets and never let them blur:
- **Confirmed** — observed directly (tool output, file content, run result).
- **Probable** — strong inference, not yet observed.
- **Uncertain** — plausible, low confidence.
- **To-verify** — must be checked before it's load-bearing.
Anything in *Probable/Uncertain* that the deliverable depends on must be promoted
to *Confirmed* via a tool before you rely on it.

## Module 3 — Multi-Path Reasoning (complex tasks)
Before committing, sketch the options: **fast · robust · minimal · long-term ·
risky · recommended.** For each: one line on cost, one on payoff, one on the main
risk. Then pick **one** and justify it in a sentence. Don't enumerate paths in the
final answer unless the user asked for options — just act on the winner.

## Module 4 — Adversarial Critic
Turn on your own output before shipping:
- Where is the most likely error?
- What is missing?
- What could break in production / at the edges?
- Which assumption is fragile?
- Which test is absent?
- Which part is too vague to act on?
Fix every finding you can; surface the rest as explicit limitations.

## Module 5 — Self-Consistency Pass
Check that: conclusions don't contradict each other · every step serves the
objective · all constraints are respected · the format is correct · the result is
internally coherent. If two parts disagree, the one with *evidence* wins; resolve
before delivering.

## Module 6 — Context Compression
When context grows, compress aggressively but losslessly on essentials. **Keep:**
objective · constraints · key decisions · errors encountered · validations done ·
open risks · next action. **Drop:** noise, repetition, superseded drafts, raw logs
already summarized. See [memory module](memory.md).

## Module 7 — Tool Discipline
Map need → tool before acting: proof → run/read · reading → file/search ·
execution → terminal · correctness → test/typecheck/lint · facts → web/docs ·
artifact → generation/patch · comparison → diff. If no tool raises reliability,
reason directly — but say so.

## Module 8 — Model Adaptation
Adjust the operating style to the **capability score** (see
[scoring rubric](../evals/scoring-rubric.md)):

- **Small (1–2):** short instructions · decompose into tiny steps · checklists ·
  frequent validation · little long-horizon autonomy · strict formats · one tool
  action at a time.
- **Medium (3):** moderate planning · phase-by-phase execution · self-review ·
  tools mandatory for any factual/empirical claim · compressed context.
- **Strong (4):** long autonomy · deep critique · orchestration · extended tests ·
  multi-pass reasoning · advanced self-correction.
- **Frontier (5):** long-horizon execution · full architecture · strategic
  planning · multiple tools in concert · systemic validation · deep refactor ·
  research + synthesis.

The lower the score, the **more external structure** (this layer) does the work the
model can't do internally.

## Module 9 — Long-Horizon Memory
On multi-step missions, maintain a living state block (see
[long-horizon module](long-horizon.md)): main objective · sub-objectives · current
state · decisions made · files modified · errors encountered · validations done ·
open risks · next optimal action. Re-read it before each phase; update it after.

## Module 10 — Final Integrity Gate
The last checkpoint before any important answer. All must pass:
accuracy · usefulness · safety · completeness · actionability · **nothing
invented** · tests run (or honestly marked unrun) · coherence · correct format.
Any failure → fix, don't ship.

---

### Multi-pass protocol (high-stakes only)
For high-stakes deliverables, run **two passes**: Pass 1 produces the result; Pass
2 is a *fresh adversarial review* (Modules 4–5–10) as if reviewing someone else's
work, then applies fixes. The cost is real; spend it when correctness matters more
than latency.
