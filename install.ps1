# Sparrow one-click installer for Windows.
#
# By default this installs the binary and STOPS — it does not launch the agent.
# Run `sparrow launch` yourself when ready, or pass -Launch to start at the end.
#
# Usage:
#   irm https://raw.githubusercontent.com/ucav/Sparrow/master/install.ps1 | iex
#   iex "& { $(irm .../install.ps1) } -Launch"            # opt in to auto-launch
#   iex "& { $(irm .../install.ps1) } -FromSource"        # build from git
#   iex "& { $(irm .../install.ps1) } -AllowUnverified"   # skip checksum (NOT recommended)

[CmdletBinding()]
param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Sparrow\bin",
    [switch]$Launch,            # default: do NOT auto-launch
    [switch]$NoLaunch,          # back-compat no-op (no-launch is already the default)
    [switch]$NoShortcut,
    [switch]$FromSource,
    [switch]$AllowUnverified    # install even without a SHA256 checksum
)

$ErrorActionPreference = "Stop"
$Repo = "ucav/Sparrow"
$Artifact = "sparrow-windows-x86_64.exe"
$BinaryPath = Join-Path $InstallDir "sparrow.exe"

function Add-UserPath {
    param([string]$PathToAdd)

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if ([string]::IsNullOrWhiteSpace($userPath)) {
        $userPath = ""
    }

    $parts = $userPath -split ';' | Where-Object { $_ -ne "" }
    if ($parts -notcontains $PathToAdd) {
        $next = if ($userPath.Trim().Length -eq 0) { $PathToAdd } else { "$userPath;$PathToAdd" }
        [Environment]::SetEnvironmentVariable("Path", $next, "User")
        $env:Path = "$env:Path;$PathToAdd"
        Write-Host "Added Sparrow to your user PATH."
    }
}

function Install-FromSource {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "Source install requires Rust/Cargo. Install Rust from https://rustup.rs or run without -FromSource."
    }
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        throw "Source install requires Git."
    }

    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("sparrow-install-" + [guid]::NewGuid())
    git clone --depth 1 "https://github.com/$Repo.git" $tmp
    Push-Location $tmp
    try {
        cargo build --release
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        Copy-Item -Force (Join-Path $tmp "target\release\sparrow.exe") $BinaryPath
    } finally {
        Pop-Location
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}

# Returns: 'ok' (verified), 'mismatch' (tampering), or 'absent' (no checksum).
function Test-ReleaseChecksum {
    param([string]$BinFile)

    $sumsUrl = "https://github.com/$Repo/releases/latest/download/$Artifact.sha256"
    $sumsFile = "$BinFile.sha256"
    try {
        Invoke-WebRequest -Uri $sumsUrl -OutFile $sumsFile -UseBasicParsing -ErrorAction Stop
    } catch {
        return 'absent'
    }

    try {
        $have = (Get-FileHash -Algorithm SHA256 -Path $BinFile).Hash.ToLower()
        $text = (Get-Content -Raw $sumsFile).ToLower()
        Remove-Item -Force $sumsFile -ErrorAction SilentlyContinue
        if ($text -match [regex]::Escape($have)) { return 'ok' } else { return 'mismatch' }
    } catch {
        Remove-Item -Force $sumsFile -ErrorAction SilentlyContinue
        return 'absent'
    }
}

function Install-FromRelease {
    $url = "https://github.com/$Repo/releases/latest/download/$Artifact"
    $tmp = "$BinaryPath.tmp"
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

    Write-Host "Downloading Sparrow release artifact: $Artifact"
    try {
        Invoke-WebRequest -Uri $url -OutFile $tmp -UseBasicParsing
    } catch {
        Remove-Item -Force $tmp -ErrorAction SilentlyContinue
        Write-Warning "Release artifact unavailable. Falling back to source build."
        Install-FromSource
        return
    }

    if (-not $AllowUnverified) {
        $verdict = Test-ReleaseChecksum -BinFile $tmp
        if ($verdict -eq 'mismatch') {
            Remove-Item -Force $tmp -ErrorAction SilentlyContinue
            throw "SHA256 MISMATCH for $Artifact - refusing to install a possibly tampered binary. Re-run with -FromSource."
        } elseif ($verdict -eq 'absent') {
            Remove-Item -Force $tmp -ErrorAction SilentlyContinue
            Write-Warning "No SHA256 checksum available for $Artifact - cannot verify integrity."
            Write-Warning "Falling back to a source build (safer than an unverified binary). Pass -AllowUnverified to override."
            Install-FromSource
            return
        }
        Write-Host "SHA256 verified."
    }

    Move-Item -Force $tmp $BinaryPath
}

function New-SparrowShortcut {
    if ($NoShortcut) {
        return
    }
    try {
        $desktop = [Environment]::GetFolderPath("Desktop")
        if ([string]::IsNullOrWhiteSpace($desktop)) {
            return
        }
        $shortcutPath = Join-Path $desktop "Sparrow.lnk"
        $shell = New-Object -ComObject WScript.Shell
        $shortcut = $shell.CreateShortcut($shortcutPath)
        $shortcut.TargetPath = $BinaryPath
        $shortcut.WorkingDirectory = [Environment]::GetFolderPath("UserProfile")
        $shortcut.Description = "Open Sparrow"
        $shortcut.Save()
        Write-Host "Desktop shortcut created: $shortcutPath"
    } catch {
        Write-Warning "Could not create desktop shortcut: $($_.Exception.Message)"
    }
}

Write-Host "Installing Sparrow into $InstallDir"
if ($FromSource) {
    Install-FromSource
} else {
    Install-FromRelease
}

Add-UserPath -PathToAdd $InstallDir
New-SparrowShortcut

Write-Host ""
Write-Host "Sparrow installed: $BinaryPath"
Write-Host "Next command: sparrow launch"

if ($Launch) {
    Write-Host ""
    Write-Host "Launching Sparrow WebView cockpit..."
    & $BinaryPath launch
}
