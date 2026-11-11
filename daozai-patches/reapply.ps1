param(
    [string]$Ref = "main",
    [string]$Install = "D:\software\Daozai\daozai.exe"
)

$ErrorActionPreference = "Stop"
$patches = $PSScriptRoot
$root = Split-Path $patches -Parent
$work = Join-Path $root ("daozai-reapply-" + (Get-Date -Format "yyyyMMdd-HHmmss"))
New-Item -ItemType Directory -Path $work -Force | Out-Null

if (Get-Process daozai -ErrorAction SilentlyContinue) {
    throw "岛仔正在运行，请先从托盘完全退出再执行"
}

$kind = if ($Ref -match '^v?\d+\.\d+') { "tags" } else { "heads" }
$url = "https://codeload.github.com/Renly1994/xiaotang-daozai/tar.gz/refs/$kind/$Ref"
$archive = Join-Path $work "src.tar.gz"
Write-Host "下载官方源码：$url"
& curl.exe -L --fail --silent --show-error --max-time 300 -o $archive $url
if ($LASTEXITCODE -ne 0) { throw "下载失败（$url）" }
& tar.exe -xzf $archive -C $work
if ($LASTEXITCODE -ne 0) { throw "解压失败" }
$src = Get-ChildItem $work -Directory | Where-Object { Test-Path (Join-Path $_.FullName "Cargo.toml") } | Select-Object -First 1
if (-not $src) { throw "找不到解压后的源码目录" }

$launcher = Get-Command py -ErrorAction SilentlyContinue
Write-Host "应用自定义补丁…"
if ($launcher) {
    & py -3 (Join-Path $patches "apply.py") $src.FullName
} else {
    & python (Join-Path $patches "apply.py") $src.FullName
}
if ($LASTEXITCODE -ne 0) {
    throw "补丁应用失败：官方源码可能已改动，请对照 PATCHES.md 更新 apply.py 后重试（源码保留在 $($src.FullName)）"
}

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$manifest = Join-Path $src.FullName "Cargo.toml"
Write-Host "编译中（首次约 3-5 分钟）…"
& cargo build --locked --release --bin daozai --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { throw "编译失败，源码保留在 $($src.FullName)" }

$exe = Join-Path $src.FullName "target\release\daozai.exe"
if (-not (Test-Path $exe)) { throw "未生成 daozai.exe" }
$old = (Get-Item $Install).VersionInfo.ProductVersion
$new = (Get-Item $exe).VersionInfo.ProductVersion
$bak = "$Install.bak-$old"
if (-not (Test-Path $bak)) { Copy-Item $Install $bak -Force }
Copy-Item $exe $Install -Force
Copy-Item (Join-Path $src.FullName "CUSTOM_BUILD.txt") (Join-Path (Split-Path $Install -Parent) "CUSTOM_BUILD.txt") -Force

Write-Host "完成：$old -> $new"
Write-Host "旧版备份：$bak"
Write-Host "补丁源码保留在：$($src.FullName)"
Write-Host "设置里「检查更新」默认关闭；确认新版本没问题后再决定是否开启。"
