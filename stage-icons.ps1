# Stage src-tauri/icons and verify they're tracked.
# Run from the repo root:  pwsh ./stage-icons.ps1
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

$lock = Join-Path '.git' 'index.lock'
if (Test-Path $lock) {
    Write-Host "Removing stale $lock"
    Remove-Item $lock -Force
}

git add src-tauri/icons
git status --short src-tauri/icons

$missing = git ls-files src-tauri/icons | Measure-Object -Line | Select-Object -ExpandProperty Lines
Write-Host "tracked icon entries: $missing"
