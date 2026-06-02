# Gateway Sessions

Gateway conversations are scoped by:

```text
gateway:<surface>:channel:<channel>:peer:<user>
```

This keeps the same person separate across platforms and channels while still
persisting continuity across restarts.

## CLI

```bash
sparrow gateway health
sparrow gateway abort <run>
sparrow sessions list
sparrow sessions export <id> [path]
sparrow sessions cleanup --older-than-days 30
```

`gateway abort` currently writes a local abort signal under the state directory.
It is intentionally explicit about this behavior until active gateway run
tracking is fully connected to the runtime interrupt channel.
