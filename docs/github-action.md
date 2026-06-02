# Sparrow GitHub Action

Run Sparrow from inside a GitHub Action — for PR reviews, CI triage, or ad-hoc
tasks triggered by `@sparrow` issue comments.

## Inputs

| Input | Required | Description |
|---|---|---|
| `mode` | no (default `run`) | `run` \| `review` \| `status` |
| `prompt` | yes for `mode: run` | The task prompt to execute |
| `pr` | no | PR number for `mode: review`; defaults to `github.event.pull_request.number` |
| `model` | no | Override the model id (e.g. `claude-opus-4-7`) |
| `allowed-tools` | no | Comma-separated tool allow-list. Empty = inherit repo config |
| `github-token` | yes | Token with `pull-requests: write` and `contents: read` |
| `sparrow-version` | no (default `master`) | Sparrow git ref to install |
| `dry-run` | no | If `true`, print the plan but never call the model or post comments |

## Behavior

The action runs as a composite action and shells out to `sparrow github
<mode>`. The CLI fails loudly when the runner is misconfigured:

- Missing `GITHUB_TOKEN` → exits non-zero with `GITHUB_TOKEN is not set…`.
- Missing `gh` CLI → exits non-zero with `\`gh\` CLI is not on PATH…`.
- `gh pr diff` failure → bubbles up `stderr` instead of silently returning an
  empty diff.

`sparrow github review --dry-run` does **not** require the environment to be
configured — it only builds and prints a `ReviewPlan` JSON so workflow steps
and tests can validate the wiring without secrets.

## Permissions

The least-privileged token grant for a PR-review workflow is:

```yaml
permissions:
  contents: read
  pull-requests: write
```

If you also want Sparrow to push fix-up commits to the PR branch, add
`contents: write`. If you use commit signing, follow the
[GitHub commit-signing guide](https://docs.github.com/en/authentication/managing-commit-signature-verification)
to provision a key and set `git config user.signingkey` before the
`Run Sparrow` step.

## Sample workflow

See [`.github/workflows/sparrow-pr-review.yml`](../.github/workflows/sparrow-pr-review.yml)
for a working example that runs on `pull_request` events and on
`@sparrow review` issue comments.

## CLI parity

Everything the action does is also available locally:

```bash
sparrow github review 123 --dry-run
sparrow github review 123 --model claude-opus-4-7 --allowed-tools fs_read,search
sparrow github status
sparrow github logs <workflow-run-id>
```

## Security notes

- The action never reads filesystem paths under `.git`, `.env`, or `.ssh`
  thanks to the sandbox policy from Phase 9.
- Secrets reaching the model are filtered by the redaction layer; the action
  does not log inputs verbatim.
- `dry-run: true` is the safe default for draft PRs — the sample workflow
  enables it automatically when `github.event.pull_request.draft` is true.
- Status: **Alpha.** The composite action and CLI are wired and tested; real
  end-to-end runs require a token with the documented permissions.
