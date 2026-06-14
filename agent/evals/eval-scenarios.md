# Eval Scenarios — measuring Fable-grade behavior

Seven scenarios. Each has a **setup**, the **axes** it probes, and a **pass bar**.
Run them on a candidate model *with the agent architecture active*, then again
without it — the delta is the value the architecture adds. Score with the
[rubric](scoring-rubric.md).

> Principle: we measure the **agent system**, not just the model. A mid model + this
> architecture should beat the bare model, and approach a strong bare model.

## Scenario 1 — Complex debug
**Setup:** a repo with a non-obvious bug (e.g. an off-by-one only triggered on
empty input, or a race behind a feature flag). **Probes:** reasoning, code, tool
use, hallucination resistance. **Pass bar:** reads the relevant code before
editing; forms an evidence-based hypothesis; fixes the root cause (not the
symptom); adds a regression test that fails-before/passes-after; confirms no
regression. **Fail signals:** guesses without reading; "should be fixed" with no
run; symptom patch.

## Scenario 2 — Refactor
**Setup:** a disorganized module with some test coverage. **Probes:** code,
reasoning, instruction following. **Pass bar:** behavior-preserving; respects
existing API + conventions; tests stay green between steps; measurably cleaner;
no scope creep. **Fail signals:** rewrites the world; breaks the public API
silently; leaves tests red.

## Scenario 3 — Long-horizon task (≈20 steps)
**Setup:** a multi-part mission (e.g. "add a feature, wire it through CLI + API +
docs, test each layer"). **Probes:** long-task stability, memory, tool use.
**Pass bar:** maintains a mission-state block; never loses the objective; respects
constraints throughout; compresses context without losing decisions; no repeated
work after a context boundary. **Fail signals:** drift, dropped constraint,
re-doing finished work, forgetting the acceptance bar.

## Scenario 4 — Research synthesis
**Setup:** "synthesize X from sources." **Probes:** hallucination resistance,
tool use, communication. **Pass bar:** cites sources; separates retrieved fact
from synthesis; flags freshness/uncertainty; **no fabricated citations or
numbers**; nuanced. **Fail signals:** invented sources/quotes, confident
un-cited claims, ignoring conflicting evidence.

## Scenario 5 — UX/UI improvement
**Setup:** "improve this interface/flow." **Probes:** reasoning, communication,
domain judgment. **Pass bar:** design coherence, accessibility considered,
interaction clarity, simplicity; concrete actionable changes, not platitudes;
verified in the preview/render where possible. **Fail signals:** generic advice,
inaccessible suggestions, unverified visual claims.

## Scenario 6 — Production feature
**Setup:** "implement feature Y end to end." **Probes:** code, tool use, testing,
safety. **Pass bar:** integrates with the existing architecture; tests (unit +
integration); security considered (input validation, no hardcoded secrets); good
DX; maintainable; verified green. **Fail signals:** stubs, untested, security
holes, doesn't actually run.

## Scenario 7 — Ambiguous prompt (stress test)
**Setup:** a vague request with missing details. **Probes:** decision engine,
assumption rule, communication. **Pass bar:** makes reasonable **stated**
assumptions and proceeds to a useful result; asks **at most one** genuinely-
blocking question, and only if truly blocked. **Fail signals:** a wall of
clarifying questions; or charging ahead on a wrong unstated assumption.

## Scoring protocol
- Run each scenario; score the 8 axes from observed behavior.
- Record: did it fabricate? did it run what it claimed? did it surface limits?
- Compute the aggregate; pick the adaptation band; re-run a sample to confirm the
  band's scaffolding produces stable behavior.
- **Regression guard:** keep a fixed scenario set and re-run on any prompt/module
  change; a drop on "hallucination resistance" or "fabricated a test result" is a
  release blocker.
