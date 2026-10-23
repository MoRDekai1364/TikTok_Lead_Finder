$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$port = 17420

function Find-Binary([string]$name, [string[]]$candidates) {
    foreach ($rel in $candidates) {
        $path = Join-Path $root $rel
        if (Test-Path $path) { return $path }
    }
    return $null
}

function Test-Core {
    try {
        $client = New-Object System.Net.Sockets.TcpClient
        $client.Connect('127.0.0.1', $port)
        $client.Close()
        return $true
    } catch {
        return $false
    }
}

$core = Find-Binary 'leadfinder.exe' @('target\release\leadfinder.exe', 'release_stage\leadfinder.exe', 'target\debug\leadfinder.exe')
$desktop = Find-Binary 'leadfinder-desktop.exe' @('ui\src-tauri\target\release\leadfinder-desktop.exe', 'release_stage\leadfinder-desktop.exe')

if (-not $core -or -not $desktop) {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.MessageBox]::Show('Release binaries were not found. Build them first with: cargo build --release -p leadfinder-cli and cargo build --release --features custom-protocol in ui\src-tauri.', 'TikTok Lead Finder') | Out-Null
    exit 1
}

$existingDesktop = Get-Process -Name 'leadfinder-desktop' -ErrorAction SilentlyContinue | Select-Object -First 1
if ($existingDesktop) {
    Add-Type -TypeDefinition @"
    using System;
    using System.Runtime.InteropServices;
    public class Win32Focus {
        [DllImport("user32.dll")]
        public static extern bool SetForegroundWindow(IntPtr hWnd);
        [DllImport("user32.dll")]
        public static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow);
    }
"@
    $hwnd = $existingDesktop.MainWindowHandle
    if ($hwnd -ne [IntPtr]::Zero) {
        [Win32Focus]::ShowWindowAsync($hwnd, 9) | Out-Null
        [Win32Focus]::SetForegroundWindow($hwnd) | Out-Null
    }
    exit 0
}

$env:PATH = [System.Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [System.Environment]::GetEnvironmentVariable('Path', 'User') + ';' + $env:PATH

$coreProcess = $null
if (-not (Test-Core)) {
    $coreProcess = Start-Process -FilePath $core -ArgumentList 'serve' -WorkingDirectory $root -WindowStyle Hidden -PassThru
    $deadline = (Get-Date).AddSeconds(45)
    while (-not (Test-Core) -and (Get-Date) -lt $deadline) {
        if ($coreProcess.HasExited) { break }
        Start-Sleep -Milliseconds 300
    }
}

$desktopProcess = Start-Process -FilePath $desktop -WorkingDirectory $root -PassThru
$desktopProcess.WaitForExit()

if ($coreProcess -and -not $coreProcess.HasExited) {
    Stop-Process -Id $coreProcess.Id -Force
}
