# Runs under Windows PowerShell 5.1 too. Downloads are mocked; Cargo must not run.
$ErrorActionPreference = 'Stop'
$Fixture = Join-Path ([IO.Path]::GetTempPath()) ('notes-install-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $Fixture 'scripts') -Force | Out-Null
Copy-Item (Join-Path $PSScriptRoot '../scripts/install.ps1') (Join-Path $Fixture 'scripts/install.ps1')
Copy-Item (Join-Path $PSScriptRoot '../herdr-plugin.toml') (Join-Path $Fixture 'herdr-plugin.toml')
$global:NotesInstallerTestCorrupt = $false
function cargo { throw 'Installer must not invoke Cargo.' }
function Invoke-WebRequest {
    param($Uri, $OutFile, [switch]$UseBasicParsing)
    if ($Uri -notmatch '^https://github.com/kormic911/herdr-notes/releases/download/v[0-9.]+/herdr-notes-v[0-9.]+-x86_64-pc-windows-msvc.exe(\.sha256)?$') { throw "Unexpected URL: $Uri" }
    if ($Uri.EndsWith('.sha256')) {
        $Binary = Join-Path (Split-Path $OutFile) ((Split-Path $Uri -Leaf) -replace '\.sha256$', '')
        $Hash = (Get-FileHash $Binary -Algorithm SHA256).Hash
        if ($global:NotesInstallerTestCorrupt) { $Hash = '0' * 64 }
        [IO.File]::WriteAllText($OutFile, $Hash)
    } else { [IO.File]::WriteAllText($OutFile, 'downloaded fixture') }
}
try {
    & (Join-Path $Fixture 'scripts/install.ps1')
    $Installed = Join-Path $Fixture 'target/release/herdr-notes.exe'
    if ((Get-Content $Installed -Raw) -ne 'downloaded fixture') { throw 'Binary not installed.' }
    [IO.File]::WriteAllText($Installed, 'keep existing binary')
    $global:NotesInstallerTestCorrupt = $true
    $Rejected = $false
    try { & (Join-Path $Fixture 'scripts/install.ps1') } catch {
        if ($_ -notmatch 'checksum mismatch') { throw }
        $Rejected = $true
    }
    if (-not $Rejected -or (Get-Content $Installed -Raw) -ne 'keep existing binary') { throw 'Corruption replaced the existing binary.' }
    Write-Output 'PASS: no Cargo needed; corrupt download rejected; existing binary preserved.'
} finally {
    if ((Split-Path -Parent ([IO.Path]::GetFullPath($Fixture))).TrimEnd('\') -ne ([IO.Path]::GetTempPath()).TrimEnd('\')) { throw 'Invalid test directory.' }
    Remove-Item -LiteralPath $Fixture -Recurse -Force
}
