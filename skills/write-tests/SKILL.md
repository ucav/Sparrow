# Skill: Write Tests

**Trigger:** add tests, write tests, test coverage, unit test, integration test, no tests

**Description:** Write comprehensive tests: nominal + edge cases + regression. Confirm tests pass for real.

## Body
When writing tests:
1. Identify the function/module under test.
2. Write tests for: nominal/happy path, edge cases (empty input, max values, null/None), error paths.
3. Include a regression test if fixing a bug.
4. Run the tests: use the `exec` tool to actually execute the test runner.
5. Report the RAW output — pass or fail counts from the real test run.
6. If tests fail: debug before claiming they pass.
7. NEVER claim "all tests pass" without running them.
