#!/usr/bin/env bash
# One-shot publisher for Sparrow's package-manager channels (macOS/Linux).
#
# Prereqs (one-time):
#   1. Install GitHub CLI:  https://cli.github.com
#   2. gh auth login        (scopes: repo, workflow)
#
# Usage:
#   ./packaging/publish-package-managers.sh 0.5.4
#
# Idempotent. Uses your existing gh session — no token in args.
set -euo pipefail

VERSION="${1:?usage: $0 <version>   e.g. $0 0.5.4}"
OWNER="${OWNER:-ucav}"
REPO="${REPO:-Sparrow}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PKG="$ROOT/packaging"

command -v gh >/dev/null || { echo "gh not found — https://cli.github.com"; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "Run: gh auth login (scopes: repo, workflow)"; exit 1; }

ensure_repo() {
  local name="$1" desc="$2"
  if ! gh repo view "$OWNER/$name" >/dev/null 2>&1; then
    echo "Creating $OWNER/$name ..."
    gh repo create "$OWNER/$name" --public --description "$desc"
  else
    echo "$OWNER/$name already exists."
  fi
}

push_file() {
  local name="$1" src="$2" path="$3" msg="$4"
  local tmp; tmp="$(mktemp -d)"
  git clone "https://github.com/$OWNER/$name.git" "$tmp" 2>/dev/null
  mkdir -p "$tmp/$(dirname "$path")"
  cp "$src" "$tmp/$path"
  ( cd "$tmp"
    git add -A
    if [ -z "$(git status --porcelain)" ]; then
      echo "  no change for $name"
    else
      git commit -m "$msg" >/dev/null
      git push origin HEAD >/dev/null
      echo "  pushed $path -> $OWNER/$name"
    fi )
  rm -rf "$tmp"
}

echo "== Homebrew tap =="
ensure_repo "homebrew-tap" "Homebrew tap for Sparrow"
push_file "homebrew-tap" "$PKG/homebrew/sparrow.rb" "Formula/sparrow.rb" "sparrow $VERSION"

echo "== Scoop bucket =="
ensure_repo "scoop-bucket" "Scoop bucket for Sparrow"
push_file "scoop-bucket" "$PKG/scoop/sparrow.json" "bucket/sparrow.json" "sparrow $VERSION"

echo "== winget =="
cat <<EOF
Use wingetcreate (it forks microsoft/winget-pkgs and opens the PR via your gh auth):

    wingetcreate update ucav.Sparrow \\
        --version $VERSION \\
        --urls https://github.com/$OWNER/$REPO/releases/download/v$VERSION/sparrow-windows-x86_64.exe \\
        --submit

Pre-built manifests are in packaging/winget/ for a manual PR.
EOF

echo
echo "Done. Users can now:"
echo "  brew install $OWNER/tap/sparrow"
echo "  scoop bucket add $OWNER https://github.com/$OWNER/scoop-bucket && scoop install sparrow"
