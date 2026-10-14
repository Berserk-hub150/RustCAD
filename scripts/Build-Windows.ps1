$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)
cargo test --workspace --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build -p rust-cad-desktop --release --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Executable: target\release\rust-cad.exe"
