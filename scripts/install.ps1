# Installation requires PowerShell 5.1, not Rust. Source builds are opt-in.
$ErrorActionPreference = 'Stop'
$PluginRoot = Split-Path -Parent $PSScriptRoot
if ($env:HERDR_NOTES_BUILD_FROM_SOURCE -eq '1') {
    Push-Location $PluginRoot
    try {
        & cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Source build failed.' }
    } finally { Pop-Location }
    return
}
$Manifest = Get-Content -LiteralPath (Join-Path $PluginRoot 'herdr-plugin.toml') -Raw
$Match = [regex]::Match($Manifest, '(?m)^version = "([0-9]+\.[0-9]+\.[0-9]+)"\r?$')
if (-not $Match.Success) { throw 'Cannot determine plugin release version.' }
$Version = $Match.Groups[1].Value
$Architecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
if ($Architecture -notin @('AMD64', 'ARM64')) { throw "Unsupported Windows architecture: $Architecture" }
# Windows ARM64 runs the x64 binary using Windows' x64 emulation.
$Asset = "herdr-notes-v$Version-x86_64-pc-windows-msvc.exe"
$Base = "https://github.com/kormic911/herdr-notes/releases/download/v$Version"
$Destination = Join-Path $PluginRoot 'target/release'
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
$Temp = Join-Path $Destination ('.download-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $Temp | Out-Null
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset" -OutFile (Join-Path $Temp $Asset)
    Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset.sha256" -OutFile (Join-Path $Temp 'checksum')
    $Checksum = (Get-Content -LiteralPath (Join-Path $Temp 'checksum') -Raw).Trim()
    if ($Checksum -notmatch '^([a-fA-F0-9]{64})$') { throw 'Invalid release checksum.' }
    # Use .NET directly: PowerShell 7 hosts can pass a PSModulePath that
    # prevents Windows PowerShell 5.1 from loading Get-FileHash.
    $Hasher = [Security.Cryptography.SHA256]::Create()
    $Stream = [IO.File]::OpenRead((Join-Path $Temp $Asset))
    try { $Actual = ([BitConverter]::ToString($Hasher.ComputeHash($Stream))).Replace('-', '') }
    finally { $Stream.Dispose(); $Hasher.Dispose() }
    if ($Actual -ine $Checksum) { throw 'Release checksum mismatch. Binary was not installed.' }
    Move-Item -LiteralPath (Join-Path $Temp $Asset) -Destination (Join-Path $Destination 'herdr-notes.exe') -Force
    Write-Output "Installed herdr-notes v$Version (Windows x64); Rust is not required."
} finally {
    # Temp is a newly created child of this plugin's target/release directory.
    if ((Split-Path -Parent ([IO.Path]::GetFullPath($Temp))) -ne [IO.Path]::GetFullPath($Destination)) { throw 'Invalid temporary directory.' }
    Remove-Item -LiteralPath $Temp -Recurse -Force
}
