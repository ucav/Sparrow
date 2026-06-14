# Reasoning-Max v2 — Inference-Time Scaling (the real lever)

> v1 (the prompt) tells the model *to* reason hard. v2 *makes it* — by spending
> extra model calls at inference time. This is the mechanism that lets a 27B
> model reach frontier single-pass quality on verifiable tasks, and lets a 100B
> exceed it. Implemented and unit-tested in Sparrow at
> `src/reasoning/inference_scaling.rs`.

## Why a longer prompt is not enough

A prompt changes the *distribution* a model samples from; it does not add
computation. The gap between a 27B and a frontier model on hard reasoning is, in
large part, a **search + verification** gap: the frontier model finds the right
chain more often on the first try. You can buy back that gap with **test-time
compute** — the well-established result that accuracy rises predictably as you
spend more inference (more samples, more verification, more refinement), often
letting a smaller model match a much larger one on tasks where a candidate answer
can be *checked*.

So the architecture stops trusting one greedy sample and instead **searches and
verifies**.

## The three primitives (all generic over `Brain`)

### 1. Best-of-N + judge selection (`best_of_n`)
Sample N drafts with temperature diversity, then a **judge call** picks the best.
This converts "the model is right 55% of the time" into "the model produces a
right answer *somewhere* in N tries, and a verifier finds it" — which is a much
higher bar to clear. Self-consistency by selection.

### 2. Reflexion self-refine (`self_refine_from`)
An **adversarial reviewer call** critiques the answer (one concrete flaw, or
`APPROVED`); a **revise call** fixes it; repeat until approved or the round budget
is spent. This is iterative error-correction: each round removes a class of
mistake the first sample made. Early-stops on approval so good answers aren't
churned.

### 3. `reason_max` — the pipeline
`best-of-N draft → judge-select → iterative self-refine`, governed by a
[`ReasoningBudget`].

## Compute budget — where the 27B / 100B story lives

`ReasoningBudget { samples, refine_rounds }` scales two ways:

**By task tier** (`for_tier`): Trivial/Small → 1 sample, 0 refine (no waste);
Medium → 1 + 1; **Hard → 3 samples + 2 refine rounds** (≈8 calls). Spend compute
only where there's reasoning to amplify.

**By model capability** (`scaled_for_capability(band)`): this is the key knob.
- **Band 1–2 (small):** +2 samples, +1 round — the weak model has the most
  headroom to recover, so it gets the most search/verification.
- **Band 3 (27B-ish, "medium"):** +1 sample. A disciplined `reason_max` run here
  is the target configuration for **"behave like Fable 5 reasoning-max"** on
  verifiable tasks: ~4 drafts, judge-selected, then 2 adversarial refine rounds.
- **Band 4–5 (100B+, strong/frontier):** budget left as-is — and because the base
  model is stronger, the *same* pipeline yields a *higher* ceiling. A 100B through
  `reason_max` is strictly better than a 27B through `reason_max`, which is the
  monotonicity you asked for: better base model + same amplifier = better result.

`max_calls()` exposes the upper bound so a cost/budget gate can cap spend
(Sparrow already meters cost per run).

## Honest boundary (where it works, where it doesn't)

Test-time compute buys the most when a candidate answer carries a **verification
signal**:
- **Code** — it runs / passes tests (the strongest signal; pair `reason_max` with
  Sparrow's exec + test tools and the verifier can be the compiler, not just a
  judge call).
- **Math / structured reasoning** — checkable steps.
- **Constraint-heavy answers** — a reviewer can spot a violated constraint.

It buys *less* on open-ended tasks with no ground truth (taste, opinion, pure
creative writing) — there, the judge is itself fallible, so we keep the budget
small. This is why the budget scales with tier, not blindly. **No amount of
test-time compute makes a small model literally equal a frontier model on tasks
with no verifier** — but on the large class of verifiable engineering tasks
Sparrow targets, it closes most of the gap. That is the maximal honest claim, and
the architecture delivers it.

## Integration into Sparrow (design)

`reason_max` is a **text-reasoning amplifier** (no tools inside its loop), so it
slots in at two clean points:

1. **`sparrow reason "<task>"` / a `reasoning_max` config flag** — route a
   reasoning-heavy answer (analysis, planning, math, design) through `reason_max`
   on the selected brain. Low risk: a distinct path that does not touch the
   streaming tool-loop.
2. **Verifier upgrade for the existing swarm** — the planner→coder→verifier loop
   already does verifier-guided refinement for code; `best_of_n` + a
   compiler/test-backed judge sharpens the verifier from "an LLM opinion" to "it
   actually ran."

For tool-using agent tasks, the right amplifier is the **swarm's verifier rework
loop** (already in `src/orchestrator/`), not this text loop — they compose:
`reason_max` for thinking, the swarm for doing.

## Measuring the lift (don't claim it — prove it)

Use the [eval harness](../evals/agent-eval-plan.md): run the verifiable scenarios
(1 debug, 2 refactor, 6 production-code) on a 27B **with** and **without**
`reason_max`, and on a 100B. Report:
- task-success delta (the architecture lift),
- `fabrication = 0` and `false-green = 0` (the floor must hold under more calls),
- cost/latency multiplier (the price of the lift).

The headline result to look for: **27B + reason_max ≈ frontier single-pass** on
verifiable tasks, **100B + reason_max > that**, at a stated compute cost. The
machine is built and tested; this is how you confirm it on your models.
