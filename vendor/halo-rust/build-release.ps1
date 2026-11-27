<#
.SYNOPSIS
    Build Rust daozai-halo.exe (wire-compatible with the Python component).
#>
param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot 'dist')
)

$ErrorActionPreference = 'Continue'
Set-Location $PSScriptRoot

& cargo build --offline --release 2>&1 | Out-Null
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$source = Join-Path $PSScriptRoot 'target\release\daozai-halo.exe'
$dest = Join-Path $OutputDirectory 'daozai-halo.exe'
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
Copy-Item -LiteralPath $source -Destination $dest -Force

$hash = (Get-FileHash -LiteralPath $dest -Algorithm SHA256).Hash.ToLowerInvariant()
$size = (Get-Item -LiteralPath $dest).Length

Write-Host "EXE: $dest"
Write-Host "SIZE: $size"
Write-Host "SHA256: $hash"
Write-Host ""
Write-Host 'component.json template:'
$manifest = @{
    version = 'x.y.z'
    size = $size
    sha256 = $hash
} | ConvertTo-Json -Compress
Write-Host $manifest
