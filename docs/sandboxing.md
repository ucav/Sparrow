# Sandboxing

## Backends

| Backend | Description |
|---|---|
| `local` | Direct execution, writes gated by autonomy + checkpoint |
| `local-hardened` | Linux namespaces + seccomp, fs allow-list, network deny |
| `docker` | Container per run, mounted workspace, capped CPU/mem |
| `ssh` | Remote execution on cloud VM |
| `singularity` | HPC/cluster container runtime |
| `modal` | Serverless container execution |
| `daytona` | Managed dev-environment sandbox |
| `vercel-sandbox` | Ephemeral serverless sandbox |
| `worktree` | Dedicated `git worktree` so mutations land on an isolated branch (`WorktreeSandbox::create`) |

## Filesystem/Network Policy

```rust
pub struct FsNetPolicy {
    pub allowed_paths: Vec<PathBuf>,
    pub allow_network: bool,
    pub denied_paths: Vec<PathBuf>,
    pub env_allowlist: Vec<String>,
}
```

- `local`: allow all paths, network allowed
- `local-hardened`: workspace only, network denied
- `docker`: workspace mounted, `--network=none` option
- All policies seed `denied_paths` with `.git`, `.env`, `.env.local`, `.ssh`,
  `id_rsa`, `id_ed25519` via `default_denied_paths()`. Both the command's
  `workdir` and every positional argument are checked against these.
- `env_allowlist` (empty by default = "pass cmd.env through"); when non-empty,
  `LocalSandbox` calls `env_clear()` before forwarding the filtered subset, so
  host env vars like `OPENAI_API_KEY` are not leaked to the child process.

## Cloud backends (Modal / Daytona / Vercel / Singularity)

These shell out to the vendor CLI. When the CLI is not installed/authenticated
they return `exit_code = 127` and a clear stderr explaining what is missing.
They never fabricate success.

## Worktree backend

`WorktreeSandbox::create(repo_root, parent_dir, branch)` runs
`git -C <repo_root> worktree add -B <branch> <parent_dir>/sparrow-<branch>`
and wraps the result in a `LocalSandbox` whose root is the new worktree path.
Construction fails clearly when `repo_root` is not a git repository.

## Sandbox Escape Tests

`tests/sandbox_policy.rs` covers:
- Workdir outside `root` → rejected with an error
- Argument referencing a protected path (`.git/config`) → rejected
- Env allowlist actually filters the child environment
- Modal/Daytona/Vercel/Singularity return honest errors when their CLI is absent
- `WorktreeSandbox::create` on a non-git directory fails with a clear message
