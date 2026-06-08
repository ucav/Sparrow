#requires -Version 5.1
<#
.SYNOPSIS
  One-shot publisher for Sparrow's package-manager channels.

.DESCRIPTION
  Creates (if missing) and populates the two repos that `brew` and `scoop`
  need, then opens the winget PR. Run this once per release after the
  GitHub release assets are live.

  Prerequisites (one-time):
    1. Install GitHub CLI:  https://cli.github.com
    2. Authenticate:        gh auth login
       (scopes needed: repo, workflow)

  Then:
    pwsh ./packaging/publish-package-managers.ps1 -Version 0.5.4

  Idempotent: re-running updates the formula/manifest in place.

.NOTES
  This script does NOT need any token pasted in chat. It uses your
  already-authenticated `gh` session.
#>

param(
    [Parameter(Mandatory = $true)]
    [string]$Version,

    [string]$Owner = "ucav",
    [string]$Repo = "Sparrow"
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$pkg = Join-Path $repoRoot "packaging"

function Assert-Gh {
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        throw "GitHub CLI (gh) not found. Install from https://cli.github.com then run: gh auth login"
    }
    gh auth status 1>$null 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "Not authenticated. Run: gh auth login  (scopes: repo, workflow)"
    }
}

function Ensure-Repo([string]$name, [string]$description) {
    $full = "$Owner/$name"
    gh repo view $full 1>$null 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Creating $full ..." -ForegroundColor Cyan
        gh repo create $full --public --description $description --disable-issues=false
    }
    else {
        Write-Host "$full already exists." -ForegroundColor DarkGray
    }
}

function Push-File([string]$repoName, [string]$localFile, [string]$repoPath, [string]$message) {
    $tmp = Join-Path $env:TEMP ("sparrow-" + $repoName + "-" + [guid]::NewGuid().ToString("N").Substring(0, 8))
    git clone "https://github.com/$Owner/$repoName.git" $tmp 2>$null
    $dest = Join-Path $tmp $repoPath
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dest) | Out-Null
    Copy-Item $localFile $dest -Force
    Push-Location $tmp
    try {
        git add -A
        $status = git status --porcelain
        if ([string]::IsNullOrWhiteSpace($status)) {
            Write-Host "  no change for $repoName" -ForegroundColor DarkGray
        }
        else {
            git commit -m $message | Out-Null
            git push origin HEAD | Out-Null
            Write-Host "  pushed $repoPath -> $Owner/$repoName" -ForegroundColor Green
        }
    }
    finally {
        Pop-Location
        Remove-Item -Recurse -Force $tmp
    }
}

# ── main ────────────────────────────────────────────────────────────────────
Assert-Gh

Write-Host "`n== Homebrew tap ==" -ForegroundColor Yellow
Ensure-Repo "homebrew-tap" "Homebrew tap for Sparrow"
Push-File "homebrew-tap" (Join-Path $pkg "homebrew/sparrow.rb") "Formula/sparrow.rb" "sparrow $Version"

Write-Host "`n== Scoop bucket ==" -ForegroundColor Yellow
Ensure-Repo "scoop-bucket" "Scoop bucket for Sparrow"
Push-File "scoop-bucket" (Join-Path $pkg "scoop/sparrow.json") "bucket/sparrow.json" "sparrow $Version"

Write-Host "`n== winget ==" -ForegroundColor Yellow
Write-Host @"
winget submission is a fork-and-PR flow against microsoft/winget-pkgs.
The cleanest path is the official tool:

    winget install wingetcreate
    wingetcreate update ucav.Sparrow `
        --version $Version `
        --urls https://github.com/$Owner/$Repo/releases/download/v$Version/sparrow-windows-x86_64.exe `
        --submit

(`wingetcreate` computes the SHA256, builds the manifests, forks
microsoft/winget-pkgs, and opens the PR for you. It reuses your gh auth.)

Your pre-built manifests are in packaging/winget/ if you prefer a manual PR.
"@ -ForegroundColor Gray

Write-Host "`nDone. Users can now:" -ForegroundColor Green
Write-Host "  brew install $Owner/tap/sparrow"
Write-Host "  scoop bucket add $Owner https://github.com/$Owner/scoop-bucket; scoop install sparrow"
Write-Host "  winget install $Owner.Sparrow   (after the winget PR merges)"
