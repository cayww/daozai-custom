param(
    [string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (!$OutputDirectory) {
    $OutputDirectory = Join-Path $PSScriptRoot '..\..\target\halo-battery'
}
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
$venv = Join-Path $OutputDirectory 'venv'
$python = Join-Path $venv 'Scripts\python.exe'
if (!(Test-Path -LiteralPath $python -PathType Leaf)) {
    py -3.12 -m venv $venv
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
}
& $python -I -c "import struct, sys; sys.exit(sys.version_info[:2] != (3, 12) or struct.calcsize('P') != 8)"
if ($LASTEXITCODE) { throw '需要 64 位 Python 3.12' }

$lock = Join-Path $PSScriptRoot 'requirements-build.txt'
& $python -I -m pip install --disable-pip-version-check --require-hashes --only-binary=:all: -r $lock
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$dist = Join-Path $OutputDirectory 'dist'
$licenses = Join-Path $OutputDirectory 'licenses'
& $python -I (Join-Path $PSScriptRoot 'collect_licenses.py') $licenses
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$upstream = Join-Path $PSScriptRoot 'upstream'
$worker = Join-Path $PSScriptRoot 'worker.py'
& $python -I -m PyInstaller --noconfirm --clean --onefile --console --noupx --name daozai-halo `
    --paths $upstream --distpath $dist --workpath (Join-Path $OutputDirectory 'temp') `
    --specpath (Join-Path $OutputDirectory 'spec') --add-data "$licenses;licenses" `
    --exclude-module tkinter --exclude-module serial --exclude-module usb --exclude-module libusb_package $worker
if ($LASTEXITCODE) { exit $LASTEXITCODE }

Get-Item -LiteralPath (Join-Path $dist 'daozai-halo.exe')
