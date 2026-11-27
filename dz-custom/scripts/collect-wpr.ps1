param(
    [int]$Seconds = 60,
    [int]$ProcessId = 0,
    [string]$OutputDirectory = "",
    [string]$PresentMonExe = "",
    [switch]$IncludeMemory,
    [switch]$NoPresentMon
)
$ErrorActionPreference = 'Stop'

$projectRoot = Split-Path $PSScriptRoot -Parent
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $projectRoot 'perf-captures' }
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'WPR needs administrator privileges. Right-click PowerShell and choose Run as Administrator.'
}

if ($ProcessId -le 0) {
    $procs = @(Get-Process -Name daozai -ErrorAction SilentlyContinue)
    if ($procs.Count -eq 0) { throw 'No running daozai.exe found. Start Daozai or pass -ProcessId.' }
    if ($procs.Count -gt 1) { throw "Multiple daozai.exe found: $($procs.Id -join ', '). Pass -ProcessId." }
    $ProcessId = $procs[0].Id
}
$target = Get-Process -Id $ProcessId -ErrorAction Stop

function Find-PresentMon {
    param([string]$Explicit)
    if ($Explicit) {
        if (-not (Test-Path -LiteralPath $Explicit)) { throw "PresentMon not found: $Explicit" }
        return (Resolve-Path -LiteralPath $Explicit).Path
    }
    $cmd = Get-Command PresentMon.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $candidates = @(
        (Join-Path $env:ProgramFiles 'PresentMon\PresentMon.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'PresentMon\PresentMon.exe'),
        (Join-Path $env:LOCALAPPDATA 'Programs\PresentMon\PresentMon.exe')
    )
    foreach ($p in $candidates) { if ($p -and (Test-Path -LiteralPath $p)) { return $p } }
    return $null
}

$wpr = Join-Path $env:WINDIR 'System32\wpr.exe'
if (-not (Test-Path -LiteralPath $wpr)) { throw "wpr.exe not found: $wpr" }

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$etl = Join-Path $OutputDirectory "daozai-$stamp.etl"
$csv = Join-Path $OutputDirectory "presentmon-$stamp.csv"

$profiles = @('CPU', 'GPU', 'DesktopComposition')
if ($IncludeMemory) { $profiles += @('Heap', 'Handle', 'VirtualAllocation') }
$startArgs = @()
foreach ($p in $profiles) { $startArgs += @('-start', $p) }
$startArgs += '-filemode'

$pm = $null
$pmProc = $null
if (-not $NoPresentMon) {
    $pm = Find-PresentMon -Explicit $PresentMonExe
    if (-not $pm) { Write-Warning 'PresentMon.exe not found; skipping frame capture. WPR still runs.' }
}

Write-Host "Target      : $($target.ProcessName) PID=$ProcessId"
Write-Host "Duration    : $Seconds seconds"
Write-Host "WPR profiles: $($profiles -join ', ')"
Write-Host "ETL         : $etl"
if ($pm) { Write-Host "PresentMon  : $csv" }

$started = $false
try {
    if ($pm) {
        $pmArgs = @(
            '--process_id', "$ProcessId",
            '--timed', "$Seconds",
            '--terminate_after_timed',
            '--date_time',
            '--write_display_metadata',
            '--no_console_stats',
            '--output_file', "`"$csv`""
        )
        $pmProc = Start-Process -FilePath $pm -ArgumentList $pmArgs -PassThru -NoNewWindow
    }

    & $wpr @startArgs
    if ($LASTEXITCODE -ne 0) { throw "wpr -start failed with exit code $LASTEXITCODE" }
    $started = $true

    Write-Host 'Capturing. Reproduce the CPU/GPU issue now...'
    Start-Sleep -Seconds $Seconds
}
finally {
    if ($started) {
        $stopArgs = @('-stop', $etl, "Daozai perf capture $stamp", '-compress', '-skipPdbGen')
        & $wpr @stopArgs
        if ($LASTEXITCODE -ne 0) { Write-Warning "wpr -stop exit code $LASTEXITCODE" }
    }
    if ($pmProc -and -not $pmProc.HasExited) {
        Stop-Process -Id $pmProc.Id -Force -ErrorAction SilentlyContinue
    }
}

Write-Host ''
Write-Host 'Captured files:'
Get-ChildItem -LiteralPath $OutputDirectory -Filter "*$stamp*" |
    Select-Object Name, @{n='MB';e={[math]::Round($_.Length / 1MB, 2)}}, LastWriteTime |
    Format-Table -AutoSize
Write-Host 'Open ETL in WPA: CPU Usage (Sampled) / GPU Activity / Desktop Composition.'
Write-Host 'Open CSV in Excel/PowerBI: MsCPUBusy / MsGPUTime / MsGPUBusy / DisplayLatency.'
