$ErrorActionPreference = 'Stop'
$binary = Join-Path $PSScriptRoot '../target/release/herdr-notes.exe'
& $binary --install-keybindings
exit $LASTEXITCODE
