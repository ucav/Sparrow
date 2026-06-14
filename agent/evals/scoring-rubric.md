# Scoring Rubric — underlying model capability (1–5 per axis)

Score each axis from observed behavior on the [eval scenarios](eval-scenarios.md).
The aggregate (weighted mean, see `config/model-capability-profile.json`) selects
the adaptation band. **Re-score per model**, and re-check if you swap models.

Anchors: **1** = unreliable / frequent failure · **3** = competent with structure ·
**5** = frontier, reliable autonomously.

## Reasoning (weight 0.20)
- **1** Loses the thread on multi-step logic; shallow.
- **3** Solid with explicit decomposition; struggles unaided on deep chains.
- **5** Deep, multi-step, abstract reasoning held reliably without scaffolding.

## Context memory (0.12)
- **1** Forgets earlier constraints within the same task.
- **3** Uses recent context well; degrades on long context.
- **5** Retains and *uses* long context accurately.

## Code (0.16)
- **1** Compiles rarely; ignores conventions; stubs.
- **3** Correct for scoped changes with guidance; runs after fixes.
- **5** Idiomatic, integrated, tested, production-grade unaided.

## Tool use (0.16)
- **1** Wrong tool, ignores errors, fabricates output.
- **3** Selects sensibly; reacts to errors with prompting.
- **5** Selects, sequences, parallelizes, and recovers autonomously.

## Hallucination resistance (0.16)
- **1** Invents facts/results confidently.
- **3** Mostly grounded; occasional unflagged guess.
- **5** Grounded; calibrates and labels uncertainty; verifies before asserting.

## Instruction following (0.08)
- **1** Drops constraints and format.
- **3** Honors most; slips on implicit constraints.
- **5** Honors explicit + implicit constraints and format precisely.

## Long-task stability (0.08)
- **1** Drifts off objective within a few steps.
- **3** Stable with an explicit mission-state block.
- **5** Holds objective across many steps / sessions without drift.

## Self-correction (0.04)
- **1** Doesn't notice its own errors.
- **3** Fixes errors when pointed at them.
- **5** Detects and fixes its own errors proactively.

## Aggregate → band
| Aggregate | Band | Effect |
|---|---|---|
| 1.0–1.9 | small | max scaffolding, tiny steps, validate each |
| 2.0–2.9 | small-medium | heavy scaffolding, frequent validation |
| 3.0–3.4 | medium | phased, self-review, tools mandatory |
| 3.5–4.4 | strong | long autonomy, adversarial multi-pass |
| 4.5–5.0 | frontier | long-horizon, systemic validation, minimal guidance |

**Invariant:** the *reliability target* is constant across bands. Only the amount
of external structure changes — lower score ⇒ the agent's architecture does more of
the work the model can't do internally.
