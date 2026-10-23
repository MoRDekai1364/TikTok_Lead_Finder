$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$launcher = Join-Path $root 'Start_TikTok_Lead_Finder.ps1'
$builder = Join-Path $root 'Build_TikTok_Lead_Finder.ps1'
$icon = Join-Path $root 'ui\src-tauri\icons\icon.ico'
$shell = New-Object -ComObject WScript.Shell

function New-LauncherShortcut([string]$path) {
    $link = $shell.CreateShortcut($path)
    $link.TargetPath = (Get-Command powershell.exe).Source
    $link.Arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$launcher`""
    $link.WorkingDirectory = $root
    $link.WindowStyle = 7
    $link.Description = 'TikTok Lead Finder'
    if (Test-Path $icon) { $link.IconLocation = $icon }
    $link.Save()
}

function New-BuilderShortcut([string]$path) {
    $link = $shell.CreateShortcut($path)
    $link.TargetPath = (Get-Command powershell.exe).Source
    $link.Arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$builder`""
    $link.WorkingDirectory = $root
    $link.WindowStyle = 1
    $link.Description = 'Build TikTok Lead Finder'
    if (Test-Path $icon) { $link.IconLocation = $icon }
    $link.Save()
}

New-LauncherShortcut (Join-Path $root 'TikTok Lead Finder.lnk')
New-BuilderShortcut (Join-Path $root 'Build TikTok Lead Finder.lnk')

if ($args -contains '-Desktop') {
    New-LauncherShortcut (Join-Path ([Environment]::GetFolderPath('Desktop')) 'TikTok Lead Finder.lnk')
    New-BuilderShortcut (Join-Path ([Environment]::GetFolderPath('Desktop')) 'Build TikTok Lead Finder.lnk')
}
