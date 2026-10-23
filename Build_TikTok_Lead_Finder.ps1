$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

$env:PATH = [System.Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path', 'User') + ';' + $env:PATH

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "      TikTok Lead Finder - Build        " -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

Write-Host "`n[1/4] Building Web UI (Vite + Svelte)..." -ForegroundColor Yellow
Push-Location "$root\ui"
try {
    npm run build
} finally {
    Pop-Location
}

Write-Host "`n[2/4] Building Backend Engine (leadfinder.exe)..." -ForegroundColor Yellow
cargo build --release --manifest-path "$root\Cargo.toml" -p leadfinder-cli

Write-Host "`n[3/4] Building Desktop App (leadfinder-desktop.exe)..." -ForegroundColor Yellow
cargo build --release --manifest-path "$root\ui\src-tauri\Cargo.toml"

Write-Host "`n[4/4] Staging release binaries..." -ForegroundColor Yellow
if (-not (Test-Path "$root\release_stage")) {
    New-Item -ItemType Directory -Path "$root\release_stage" | Out-Null
}
Copy-Item -Path "$root\target\release\leadfinder.exe" -Destination "$root\release_stage\leadfinder.exe" -Force
Copy-Item -Path "$root\ui\src-tauri\target\release\leadfinder-desktop.exe" -Destination "$root\release_stage\leadfinder-desktop.exe" -Force

$iscc = 'C:\Program Files\Inno Setup 7\ISCC.exe'
if (Test-Path $iscc) {
    Write-Host "`n[Optional] Compiling Windows Installer (Inno Setup)..." -ForegroundColor Yellow
    & $iscc "$root\Setup_TikTok_Lead_Finder_Official_Release.iss"
}

Write-Host "`n========================================" -ForegroundColor Green
Write-Host "   Build Complete! All binaries ready.  " -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host "`nPress any key to close this window..."
$null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')
