﻿﻿<#
.SYNOPSIS
    用 Inno Setup 7 编译岛仔定制版安装包（installer\daozai.iss）。

.DESCRIPTION
    自动探测 ISCC.exe（优先 D:\software\Inno Setup 7\ISCC.exe，其次注册表/PATH/常见路径），
    编译 daozai.iss，输出安装包路径与 sha256。构建 daozai.exe / daozai-bridge.exe 需先执行：
      cargo build --locked --release --manifest-path dz-phone\Cargo.toml --bin daozai --bin daozai-bridge

.PARAMETER InnoCompiler
    显式指定 ISCC.exe 路径，跳过自动探测。

.PARAMETER SkipBuild
    预留开关：本脚本只负责 Inno Setup 编译，构建始终需单独执行（保留参数以免调用方传参报错）。

.EXAMPLE
    .\build-innosetup.ps1
    .\build-innosetup.ps1 -InnoCompiler 'D:\software\Inno Setup 7\ISCC.exe'
#>
param(
    [string]$InnoCompiler,
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'

$installerDir = $PSScriptRoot
$projectRoot = Split-Path $installerDir -Parent
$iss = Join-Path $installerDir 'daozai.iss'
$exe = Join-Path $projectRoot 'dz-phone\target\release\daozai.exe'
$bridge = Join-Path $projectRoot 'dz-phone\target\release\daozai-bridge.exe'

# ---------- 探测 ISCC ----------
function Find-ISCC {
    param([string]$Explicit)
    $candidates = @()
    if ($Explicit) { $candidates += $Explicit }
    if ($env:INNO_SETUP_HOME) { $candidates += (Join-Path $env:INNO_SETUP_HOME 'ISCC.exe') }
    # 本机历史固定路径（HANDOFF 记载，Inno Setup 7）：优先放前面
    $candidates += 'D:\software\Inno Setup 7\ISCC.exe'
    $candidates += 'D:\software\Inno Setup 6\ISCC.exe'
    $candidates += (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe')
    $candidates += (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
    foreach ($key in @(
        'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
        'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
    )) {
        Get-ItemProperty $key -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -like 'Inno Setup*' -and $_.InstallLocation } |
            ForEach-Object { $candidates += (Join-Path $_.InstallLocation 'ISCC.exe') }
    }
    $fromPath = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($fromPath) { $candidates += $fromPath.Source }

    foreach ($c in ($candidates | Where-Object { $_ } | Select-Object -Unique)) {
        if (Test-Path -LiteralPath $c) { return $c }
    }
    return $null
}

$iscc = Find-ISCC -Explicit $InnoCompiler
if (-not $iscc) {
    throw "未找到 ISCC.exe。请用 -InnoCompiler 指定，或设置 INNO_SETUP_HOME。历史路径：D:\software\Inno Setup 7\ISCC.exe"
}

# ---------- 前置检查 ----------
foreach ($f in @($iss, $exe, $bridge)) {
    if (-not (Test-Path -LiteralPath $f)) { throw "缺少文件：$f（先跑 cargo build --release 双 bin）" }
}
Push-Location $installerDir
try {
    Write-Host "ISCC: $iscc"
    Write-Host "工程: $iss"
    & $iscc $iss
    if ($LASTEXITCODE -ne 0) { throw "Inno Setup 编译失败（exit $LASTEXITCODE）" }

    # 从 .iss 里读 OutputBaseFilename / OutputDir 以定位产物
    $raw = Get-Content $iss -Raw
    $build = [regex]::Match($raw, '(?m)#define\s+MyAppBuild\s+"([^"]+)"').Groups[1].Value
    $outBase = [regex]::Match($raw, '(?m)^OutputBaseFilename=(.+)$').Groups[1].Value.Trim()
    # .iss 里常写 OutputBaseFilename=daozai-setup-{#MyAppBuild}，需要展开 ISPP 宏
    $outBase = $outBase.Replace('{#MyAppBuild}', $build)
    $outDir = [regex]::Match($raw, '(?m)^OutputDir=(.+)$').Groups[1].Value.Trim().Replace('{#MyAppBuild}', $build)
    if (-not $outBase) { $outBase = "daozai-setup-$build" }
    # OutputDir 相对 .iss 所在目录（installer），ISCC 的工作目录也是 installer
    $outputDirResolved = if ($outDir) { [IO.Path]::GetFullPath((Join-Path $installerDir $outDir)) } else { $installerDir }
    $setup = Join-Path $outputDirResolved "$outBase.exe"
    if (-not (Test-Path -LiteralPath $setup)) { throw "未找到产物：$setup" }

    $hash = (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant()
    $exeHash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
    $bridgeHash = (Get-FileHash -LiteralPath $bridge -Algorithm SHA256).Hash.ToLowerInvariant()
    Write-Host "包: $setup"
    Write-Host ("包 sha256 : {0}" -f $hash.Substring(0, 16))
    Write-Host ("exe sha256: {0}" -f $exeHash.Substring(0, 16))
    Write-Host ("bridge    : {0}" -f $bridgeHash.Substring(0, 16))
    Write-Output $setup
} finally { Pop-Location }
