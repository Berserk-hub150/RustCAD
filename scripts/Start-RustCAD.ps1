$ErrorActionPreference = "Stop"
$projectPath = Split-Path -Parent $PSScriptRoot
Set-Location $projectPath
if (Test-Path ".\windows\rust-cad.exe") {
    Start-Process -FilePath ".\windows\rust-cad.exe" -WorkingDirectory $projectPath
    exit 0
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Installa Rust da https://rustup.rs/ e segui la richiesta dei C++ Build Tools."
    Write-Host "Poi riapri PowerShell e riesegui questo file."
    exit 1
}
cargo run -p rust-cad-desktop --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
