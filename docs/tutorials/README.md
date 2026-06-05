# Sparrow Video Tutorials

These tutorials are checked-in MP4 walkthroughs for the public docs site. They
are intentionally short, silent, and chaptered so they can load quickly from a
static site while still showing the actual product workflows.

## First Launch In 5 Minutes

Video: [`first-launch.mp4`](first-launch.mp4)

Chapters:

- `00:00` Install and build Sparrow locally.
- `00:06` Run `sparrow setup` and configure provider credentials.
- `00:12` Start `sparrow console` and open the WebView cockpit.
- `00:18` Send the first routed task and inspect route, token, and cost meters.

Commands:

```bash
cargo build
sparrow setup
sparrow console
sparrow run "explain this repository and list the risky files"
```

## Safe Edits And Rewind

Video: [`safe-edits-rewind.mp4`](safe-edits-rewind.mp4)

Chapters:

- `00:00` Keep autonomy supervised before mutating tools run.
- `00:06` Let Sparrow checkpoint the workspace before edits.
- `00:12` Review generated diffs by hunk in the WebView cockpit.
- `00:18` Use `sparrow rewind <checkpoint-id>` to restore the tree.

Commands:

```bash
sparrow checkpoint list
sparrow run "fix the failing test"
sparrow rewind <checkpoint-id>
```

## Memory Graph + Browser-Use

Video: [`memory-graph-browser.mp4`](memory-graph-browser.mp4)

Chapters:

- `00:00` Upsert persistent graph entities into SQLite memory.
- `00:06` Search graph neighbors between sessions.
- `00:12` Optionally sync the local graph to Neo4j.
- `00:18` Install Playwright and use browser/computer tools for screenshot,
  click, and type workflows.

Commands:

```bash
sparrow memory graph upsert-node project:sparrow Sparrow --kind project
sparrow memory graph search sparrow
sparrow memory graph sync-neo4j
npm install
npm run browser:install
```
