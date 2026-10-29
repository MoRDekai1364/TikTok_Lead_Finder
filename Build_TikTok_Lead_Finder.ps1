param(
    [switch]$ResetFrames
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location $root

$env:PATH = [System.Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path', 'User') + ';' + $env:PATH

function Assert-Exit([string]$step) {
    if ($LASTEXITCODE -ne 0) {
        Write-Host "$step failed with exit code $LASTEXITCODE" -ForegroundColor Red
        throw "$step failed"
    }
}

function Show-Stamp([string]$path) {
    if (Test-Path $path) {
        $item = Get-Item $path
        Write-Host ("  {0}  ->  {1}" -f $item.FullName, $item.LastWriteTime) -ForegroundColor Gray
    } else {
        Write-Host "  missing: $path" -ForegroundColor Red
    }
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "      TikTok Lead Finder - Build        " -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan

try {
    Write-Host "`n[1/7] Stopping running copies (desktop app and hidden backend)..." -ForegroundColor Yellow
    $running = Get-Process -Name 'leadfinder', 'leadfinder-desktop' -ErrorAction SilentlyContinue
    if ($running) {
        $running | ForEach-Object { Write-Host ("  stopping {0} (PID {1})" -f $_.Name, $_.Id) -ForegroundColor Gray }
        $running | Stop-Process -Force
        Start-Sleep -Seconds 2
    } else {
        Write-Host "  nothing was running" -ForegroundColor Gray
    }

    Write-Host "`n[2/7] Installing OCR Python packages (non-fatal)..." -ForegroundColor Yellow
    $python = Get-Command python -ErrorAction SilentlyContinue
    if ($python) {
        try {
            & python -m pip install --quiet --disable-pip-version-check huggingface_hub rapidocr_onnxruntime
            if ($LASTEXITCODE -ne 0) {
                Write-Host "  pip finished with errors, OCR will fall back to the default RapidOCR model" -ForegroundColor DarkYellow
            }
        } catch {
            Write-Host "  pip failed: $($_.Exception.Message)" -ForegroundColor DarkYellow
        }
    } else {
        Write-Host "  python not found, skipping" -ForegroundColor DarkYellow
    }

    Write-Host "`n[3/7] Building Web UI (Vite + Svelte)..." -ForegroundColor Yellow
    Push-Location "$root\ui"
    try {
        npm run build
        Assert-Exit 'Web UI build'
    } finally {
        Pop-Location
    }

    $labelEscaped = '\u0417\u0430\u0441\u0435\u0447\u0435\u043d\u0438'
    $labelLiteral = [regex]::Unescape($labelEscaped)
    $labelHit = Get-ChildItem -Path "$root\ui\dist" -Recurse -Include '*.js' -ErrorAction SilentlyContinue |
        Select-String -Pattern $labelLiteral, $labelEscaped -SimpleMatch -Encoding UTF8 -List
    if ($labelHit) {
        Write-Host "  new tab label found in ui\dist" -ForegroundColor Green
    } else {
        Write-Host "  WARNING: new tab label not found in ui\dist, check ui\src\lib\i18n.ts" -ForegroundColor Red
    }

    Write-Host "`n[4/7] Building Backend Engine (leadfinder.exe)..." -ForegroundColor Yellow
    cargo build --release --manifest-path "$root\Cargo.toml" -p leadfinder-cli
    Assert-Exit 'Backend build'

    Write-Host "`n[5/7] Building Desktop App (leadfinder-desktop.exe)..." -ForegroundColor Yellow
    (Get-Item "$root\ui\src-tauri\src\main.rs").LastWriteTime = Get-Date
    cargo build --release --manifest-path "$root\ui\src-tauri\Cargo.toml" --features custom-protocol
    Assert-Exit 'Desktop build'

    Write-Host "`n[6/7] Staging release binaries..." -ForegroundColor Yellow
    if (-not (Test-Path "$root\release_stage")) {
        New-Item -ItemType Directory -Path "$root\release_stage" | Out-Null
    }
    Copy-Item -Path "$root\target\release\leadfinder.exe" -Destination "$root\release_stage\leadfinder.exe" -Force
    Copy-Item -Path "$root\ui\src-tauri\target\release\leadfinder-desktop.exe" -Destination "$root\release_stage\leadfinder-desktop.exe" -Force
    if (Test-Path "$root\ui\src-tauri\target\release\WebView2Loader.dll") {
        Copy-Item -Path "$root\ui\src-tauri\target\release\WebView2Loader.dll" -Destination "$root\release_stage\WebView2Loader.dll" -Force
    }

    Write-Host "`n[7/7] Verifying build timestamps..." -ForegroundColor Yellow
    Show-Stamp "$root\target\release\leadfinder.exe"
    Show-Stamp "$root\ui\src-tauri\target\release\leadfinder-desktop.exe"
    Show-Stamp "$root\release_stage\leadfinder.exe"
    Show-Stamp "$root\release_stage\leadfinder-desktop.exe"

    if ($ResetFrames) {
        Write-Host "`n[Optional] Clearing cached video frames so OCR re-runs on next processing..." -ForegroundColor Yellow
        if (Test-Path "$root\data") {
            Get-ChildItem -Path "$root\data" -Recurse -Directory -Filter 'frames' -ErrorAction SilentlyContinue |
                ForEach-Object {
                    Write-Host "  removing $($_.FullName)" -ForegroundColor Gray
                    Remove-Item -Path $_.FullName -Recurse -Force
                }
        } else {
            Write-Host "  data folder not found at $root\data" -ForegroundColor DarkYellow
        }
    }

    $iscc = 'C:\Program Files\Inno Setup 7\ISCC.exe'
    if (Test-Path $iscc) {
        Write-Host "`n[Optional] Compiling Windows Installer (Inno Setup)..." -ForegroundColor Yellow
        & $iscc "$root\Setup_TikTok_Lead_Finder_Official_Release.iss"
        Assert-Exit 'Installer build'
    }

    Write-Host "`n========================================" -ForegroundColor Green
    Write-Host "   Build Complete! All binaries ready.  " -ForegroundColor Green
    Write-Host "========================================" -ForegroundColor Green
} catch {
    Write-Host "`n========================================" -ForegroundColor Red
    Write-Host "   Build FAILED" -ForegroundColor Red
    Write-Host "   $($_.Exception.Message)" -ForegroundColor Red
    Write-Host "========================================" -ForegroundColor Red
}

Write-Host "`nPress any key to close this window..."
$null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')
