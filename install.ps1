# envsync installer for Windows
# Run: powershell -ExecutionPolicy Bypass -File install.ps1
# Or:  irm https://raw.githubusercontent.com/DukeeTheProgrammer/envsync/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$Repo = "DukeeTheProgrammer/envsync"
$Binary = "envsync"
$InstallDir = "$env:USERPROFILE\.local\bin"

function Write-Info  { param($Msg) Write-Host ">>> $Msg" -ForegroundColor Cyan }
function Write-Ok    { param($Msg) Write-Host ">>> $Msg" -ForegroundColor Green }
function Write-Err   { param($Msg) Write-Host "error: $Msg" -ForegroundColor Red; exit 1 }

function Get-Platform {
    $arch = [System.Environment]::Is64BitOperatingSystem
    $os = if ($IsWindows -or $env:OS -eq "Windows_NT") { "windows" } else { "unknown" }
    $cpu = if ($arch) { "x86_64" } else { "x86" }
    return @{ os = $os; arch = $cpu; target = "$cpu-pc-windows-msvc" }
}

function Install-Binary {
    param($Src)
    if (-not (Test-Path $Src)) {
        Write-Err "Binary not found at $Src"
    }

    if (-not (Test-Path $InstallDir)) {
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    Copy-Item $Src "$InstallDir\$Binary.exe" -Force

    # Add to PATH if not already there
    $currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
    if ($currentPath -notlike "*$InstallDir*") {
        [Environment]::SetEnvironmentVariable("PATH", "$InstallDir;$currentPath", "User")
        $env:PATH = "$InstallDir;$env:PATH"
        Write-Info "Added $InstallDir to your PATH"
        Write-Info "Restart your terminal or run: `$env:PATH = `"$InstallDir;`$env:PATH`""
    }

    Write-Ok "Installed $Binary to $InstallDir\$Binary.exe"
    Write-Host ""
    Write-Ok "Run 'envsync --help' to get started"
}

function Try-Download {
    $platform = Get-Platform
    $tag = try {
        (Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -ErrorAction SilentlyContinue).tag_name
    } catch { $null }

    if (-not $tag) { return $false }

    $url = "https://github.com/$Repo/releases/download/$tag/$Binary-$($platform.target).zip"
    Write-Info "Downloading $Binary $tag for $($platform.target)..."

    $tmpDir = Join-Path $env:TEMP "envsync-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null

    try {
        $zipPath = Join-Path $tmpDir "$Binary.zip"
        Invoke-WebRequest -Uri $url -OutFile $zipPath -ErrorAction Stop
        Expand-Archive -Path $zipPath -DestinationPath $tmpDir -Force
        $exePath = Join-Path $tmpDir "$Binary.exe"
        if (Test-Path $exePath) {
            Install-Binary $exePath
            return $true
        }
    } catch {
        Write-Info "Download failed, will try building from source..."
    } finally {
        Remove-Item $tmpDir -Recurse -Force -ErrorAction SilentlyContinue
    }
    return $false
}

function Build-FromSource {
    Write-Info "Building from source..."

    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Err "Rust/cargo not found. Install it: https://rustup.rs"
    }

    $scriptDir = Split-Path -Parent $MyInvocation.ScriptName
    $cargoToml = Join-Path $scriptDir "Cargo.toml"

    if (Test-Path $cargoToml) {
        Write-Info "Building from local source..."
        Push-Location $scriptDir
        cargo build --release
        $exePath = Join-Path $scriptDir "target\release\$Binary.exe"
        if (Test-Path $exePath) {
            Install-Binary $exePath
        } else {
            Write-Err "Build failed"
        }
        Pop-Location
    } else {
        Write-Err "Could not find local source. Install Rust and run: cargo install $Binary"
    }
}

# --- Main ---
Write-Host ""
Write-Host "envsync installer" -ForegroundColor White
Write-Host ""

if (Test-Path "$InstallDir\$Binary.exe") {
    Write-Ok "$Binary is already installed at $InstallDir\$Binary.exe"
    Write-Info "Run 'envsync --help' or 'envsync --version'"
    exit 0
}

if (Try-Download) { exit 0 }
Build-FromSource
