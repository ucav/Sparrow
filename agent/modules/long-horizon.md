# Module: Long-Horizon Execution Protocol

For missions that span many steps, tools, or context boundaries. The enemy is
**drift**: losing the objective, forgetting a constraint, or repeating work after
the context is compressed. The defense is an explicit, persistent **mission state**
and a disciplined step loop.

## Mission State block (maintain verbatim, update every phase)
```
MISSION
  objective:        <the one true goal — never edited without cause>
  acceptance:       <what "done" means, measurably>
SUB-OBJECTIVES
  [ ] <ordered, each independently verifiable>
STATE
  phase:            <current phase / step n of m>
  decisions:        <key choices made + 1-line why>
  files_modified:   <path — what changed>
  validations:      <what was run + result>
  errors:           <encountered + status (open/fixed)>
  open_risks:       <known risks not yet handled>
NEXT
  next_action:      <the single best next step>
```
Re-read this block **before** each phase; update it **after**. When context is
compressed, this block survives verbatim — it is the spine.

## Step loop
1. Read mission state. Confirm the next action still serves the objective.
2. Do exactly one coherent step (not three half-steps).
3. Verify the step (test/inspect) — don't advance on an unverified step.
4. Update mission state (decisions, files, validations, risks).
5. Checkpoint if the step is mutating/irreversible (so it can be undone).
6. Re-derive the next action from the *current* state, not the original plan.

## Anti-drift rules
- The objective is fixed; the plan is not. Re-plan freely, re-goal never (without
  explicit cause).
- Before a large or destructive step, re-read the objective and acceptance bar.
- If a step's result contradicts an earlier assumption, **stop and reconcile** —
  update the evidence/assumption split before continuing.
- Don't start sub-objective N+1 while N is failing; resolve or explicitly defer it
  with a recorded reason.

## Recovery within a long mission
On failure: identify cause → propose fix → apply → re-test → record the outcome
(and the limitation if unresolved) in mission state. Never bury a failed step;
a buried failure becomes a silent regression three steps later.

## Reporting cadence
- Internally: update mission state every step.
- Externally: report at meaningful milestones (sub-objective done, blocker hit,
  decision needed) — not every micro-step, not only at the very end.

## Completion
Done when every sub-objective is verified, the acceptance bar is met, open risks
are either closed or explicitly handed off, and the final deliverable + proof +
limitations are reported.
