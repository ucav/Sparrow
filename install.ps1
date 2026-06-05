# Sparrow one-click installer for Windows.
# Usage:
#   irm https://raw.githubusercontent.com/ucav/Sparrow/master/install.ps1 | iex
#   iex "& { $(irm https://raw.githubusercontent.com/ucav/Sparrow/master/install.ps1) } -NoLaunch"

[CmdletBinding()]
param(
    [string]$InstallDir = "$env:LOCALAPPDATA\Sparrow\bin",
    [switch]$NoLaunch,
    [switch]$FromSource
)

$ErrorActionPreference = "Stop"
$Repo = "ucav/Sparrow"
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

function Install-FromRelease {
    $url = "https://github.com/$Repo/releases/latest/download/sparrow-windows-x86_64.exe"
    $tmp = "$BinaryPath.tmp"
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

    Write-Host "Downloading Sparrow release artifact: sparrow-windows-x86_64.exe"
    try {
        Invoke-WebRequest -Uri $url -OutFile $tmp -UseBasicParsing
        Move-Item -Force $tmp $BinaryPath
    } catch {
        Remove-Item -Force $tmp -ErrorAction SilentlyContinue
        Write-Warning "Release artifact unavailable. Falling back to source build."
        Install-FromSource
    }
}

Write-Host "Installing Sparrow into $InstallDir"
if ($FromSource) {
    Install-FromSource
} else {
    Install-FromRelease
}

Add-UserPath -PathToAdd $InstallDir

Write-Host ""
Write-Host "Sparrow installed: $BinaryPath"
Write-Host "Next command: sparrow launch"

if (-not $NoLaunch) {
    Write-Host ""
    Write-Host "Launching Sparrow WebView cockpit..."
    & $BinaryPath launch
}
