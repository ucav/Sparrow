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

## Filesystem/Network Policy

```rust
pub struct FsNetPolicy {
    pub allowed_paths: Vec<PathBuf>,
    pub allow_network: bool,
}
```

- `local`: allow all paths, network allowed
- `local-hardened`: workspace only, network denied
- `docker`: workspace mounted, `--network=none` option

## Sandbox Escape Tests

Tests verify:
- Write outside workspace → blocked
- Network access when denied → blocked
- Sandbox escape signal → halt + notify
