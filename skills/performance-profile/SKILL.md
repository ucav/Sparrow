# Skill: Performance Profile

**Trigger:** slow, performance, optimize, profiling, bottleneck, faster

**Description:** Measure before optimizing. Never optimize blindly.

## Body
When investigating performance:
1. MEASURE: Profile the current code. Use appropriate tools (cargo bench, pytest-benchmark, time).
2. IDENTIFY: Find the actual bottleneck from profile data, not intuition.
3. BASELINE: Record the current performance as a baseline.
4. OPTIMIZE: Make ONE change, re-measure. Compare to baseline.
5. If no improvement: revert. Try a different approach.
6. NEVER: Optimize without measuring first. Guess the bottleneck. Optimize prematurely.
