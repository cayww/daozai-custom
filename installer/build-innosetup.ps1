<#
.SYNOPSIS
    岛仔定制版唯一打包入口：构建 → Inno Setup 编译 → 发布资源（安装包 + zip + SHA256SUMS）。

.DESCRIPTION
    默认做完整构建；-SkipBuild 时复用已有的 target/<profile> 产物。
    唯一安装器脚本是 installer\daozai.iss（Inno Setup），NSIS 链路已移除。
    产物固定写到 dist/：
      Daozai-Setup-v<version>-x64.exe
      Daozai-v<version>-windows-x64.zip
      SHA256SUMS.txt
    zip 的名字与内部结构对齐更新模块（updates.rs::release_info）。

.PARAMETER InnoCompiler
    显式指定 ISCC.exe 路径，跳过自动探测。

.PARAMETER SkipBuild
    跳过 cargo 构建，只复用 target/<profile> 下已有产物。

.PARAMETER CargoProfile
    release（默认，正式发版）或 dev-release（自测）。

.EXAMPLE
    .\build-innosetup.ps1
    .\build-innosetup.ps1 -CargoProfile dev-release
    .\build-innosetup.ps1 -SkipBuild
#>
param(
    [string]$InnoCompiler,
    [switch]$SkipBuild,
    [ValidateSet('release','dev-release')]
    [string]$CargoProfile = 'release'
)
$ErrorActionPreference = 'Stop'

$installerDir = $PSScriptRoot
$projectRoot = Split-Path $installerDir -Parent
$iss = Join-Path $installerDir 'daozai.iss'
$targetDir = Join-Path $projectRoot ('target\' + $CargoProfile)
$exe = Join-Path $targetDir 'daozai.exe'
$bridge = Join-Path $targetDir 'daozai-bridge.exe'
$haloExe = Join-Path $projectRoot ('vendor\halo-rust\target\' + $CargoProfile + '\daozai-halo.exe')
$dist = Join-Path $projectRoot 'dist'

$cargoVersion = [regex]::Match((Get-Content (Join-Path $projectRoot 'Cargo.toml') -Raw), '(?m)^version = "([^"]+)"').Groups[1].Value
if (-not $cargoVersion) { throw '无法从 Cargo.toml 读取版本号' }

# ---------- 构建 ----------
if (-not $SkipBuild) {
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    $rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' }
    # 发布版不嵌入本机路径；CARGO_ENCODED_RUSTFLAGS 覆盖仓库/全局 rustflags，故带上 crt-static。
    $env:CARGO_ENCODED_RUSTFLAGS = @(
        '-C', 'target-feature=+crt-static',
        "--remap-path-prefix=$cargoHome=cargo",
        "--remap-path-prefix=$rustupHome=rustup",
        "--remap-path-prefix=$projectRoot=daozai",
        '-C', 'link-arg=/PDBALTPATH:%_PDB%'
    ) -join [char]0x1f
    [Environment]::SetEnvironmentVariable('DAOZAI_TEST_BUILD', $null)
    $previousEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        Push-Location $projectRoot
        & cargo build --locked --profile $CargoProfile --features sdk-js,sdk-material --bin daozai --bin daozai-bridge
        if ($LASTEXITCODE -ne 0) { throw '主程序构建失败' }
        & cargo build --locked --profile $CargoProfile --manifest-path (Join-Path $projectRoot 'vendor\halo-rust\Cargo.toml')
        if ($LASTEXITCODE -ne 0) { throw '内置电量组件构建失败' }
    } finally {
        Pop-Location
        $ErrorActionPreference = $previousEap
        [Environment]::SetEnvironmentVariable('CARGO_ENCODED_RUSTFLAGS', $null)
    }
}

# ---------- 探测 ISCC ----------
function Find-ISCC {
    param([string]$Explicit)
    $candidates = @()
    if ($Explicit) { $candidates += $Explicit }
    if ($env:INNO_SETUP_HOME) { $candidates += (Join-Path $env:INNO_SETUP_HOME 'ISCC.exe') }
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
foreach ($f in @($iss, $exe, $bridge, $haloExe)) {
    if (-not (Test-Path -LiteralPath $f)) {
        throw "缺少文件：$f（去掉 -SkipBuild 重新运行，或先手动构建对应 profile）"
    }
}
& (Join-Path $projectRoot 'scripts\check-runtime.ps1') -Executable $exe
$exeVersion = (Get-Item -LiteralPath $exe).VersionInfo.FileVersion
if ($exeVersion -ne $cargoVersion -and $exeVersion -ne "$cargoVersion.0") {
    throw "主程序版本不匹配：$exeVersion != $cargoVersion"
}

New-Item -ItemType Directory -Path $dist -Force | Out-Null
$setup = Join-Path $dist "Daozai-Setup-v$cargoVersion-x64.exe"
$packageName = "Daozai-v$cargoVersion-windows-x64"
$staging = Join-Path $dist $packageName
$archive = Join-Path $dist "$packageName.zip"

# ---------- Inno Setup 编译 ----------
Push-Location $installerDir
try {
    Write-Host "ISCC: $iscc"
    Write-Host "工程: $iss"
    Write-Host "产物: $setup"
    # ISCC 会把既有警告写到 stderr；PS 5.1 下临时放宽，只按退出码判断。
    $previousEap = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    try {
        & $iscc "/DTargetProfile=$CargoProfile" "/DMyAppVersion=$cargoVersion" $iss
        $compileExit = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousEap
    }
    if ($compileExit -ne 0) { throw "Inno Setup 编译失败（exit $compileExit）" }
} finally { Pop-Location }
# .iss 固定输出 daozai-setup-custom-isolated.exe，复制成历史发布命名。
$raw = Get-Content $iss -Raw
$build = [regex]::Match($raw, '(?m)#define\s+MyAppBuild\s+"([^"]+)"').Groups[1].Value
$issSetup = Join-Path $installerDir "daozai-setup-$build.exe"
if (-not (Test-Path -LiteralPath $issSetup)) { throw "未找到 Inno 产物：$issSetup" }
if (Test-Path -LiteralPath $setup) { [IO.File]::Delete($setup) }
Copy-Item -LiteralPath $issSetup -Destination $setup

# ---------- 便携 zip（更新模块依赖这个名字和内部结构） ----------
if (Test-Path -LiteralPath $staging) { [IO.Directory]::Delete($staging, $true) }
if (Test-Path -LiteralPath $archive) { [IO.File]::Delete($archive) }
New-Item -ItemType Directory -Path $staging -Force | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $staging 'daozai.exe')
@('LICENSE', 'THIRD_PARTY_NOTICES.txt') | ForEach-Object {
    Copy-Item -LiteralPath (Join-Path $projectRoot $_) -Destination $staging -Recurse
}
Copy-Item -LiteralPath (Join-Path $installerDir '收集岛仔诊断.cmd') -Destination $staging
Compress-Archive -LiteralPath $staging -DestinationPath $archive

@($setup, $archive) | ForEach-Object {
    $checksum = (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
    "$checksum  $([IO.Path]::GetFileName($_))"
} | Set-Content -LiteralPath (Join-Path $dist 'SHA256SUMS.txt') -Encoding ascii

Write-Host ("安装器 sha256: {0}" -f (Get-FileHash -LiteralPath $setup -Algorithm SHA256).Hash.ToLowerInvariant())
Write-Host ("zip     sha256: {0}" -f (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant())
Write-Output $setup
Write-Output $archive
