<#
.SYNOPSIS
岛仔通用状态总线写入器（PowerShell 版，无需 Python）。

.EXAMPLE
.\send_status.ps1 -Id build -Text "构建中 55%" -Detail "cargo build --release" -Progress 0.55 -Color blue
.\send_status.ps1 -Id build -Remove
.\send_status.ps1 -List
.\send_status.ps1 -Clear
#>
param(
    [string]$Id,
    [string]$Text,
    [string]$Detail = "",
    [double]$Seconds = 0,
    [double]$Progress = -1,
    [ValidateSet("", "red", "green", "blue", "amber", "gray")][string]$Color = "",
    [string]$Click = "",
    [switch]$Remove,
    [switch]$List,
    [switch]$Clear
)

$ErrorActionPreference = "Stop"
$folder = Join-Path $env:APPDATA "Daozai"
if (-not (Test-Path -LiteralPath $folder)) { New-Item -ItemType Directory -Path $folder -Force | Out-Null }
$path = Join-Path $folder "island_status.json"
$enc = New-Object System.Text.UTF8Encoding($false)

function Get-Key($item) {
    if ($item.PSObject.Properties.Name -contains "id" -and $item.id) { return [string]$item.id }
    if ($item.PSObject.Properties.Name -contains "text") { return [string]$item.text }
    return ""
}

function Read-Items {
    if (-not (Test-Path -LiteralPath $path)) { return @() }
    try {
        $raw = Get-Content -LiteralPath $path -Raw -Encoding UTF8
        $value = ConvertFrom-Json -InputObject $raw
    } catch { return @() }
    if ($null -eq $value) { return @() }
    return @($value)
}

function Write-Items($items) {
    $json = ConvertTo-Json -InputObject @($items) -Depth 6 -Compress
    if (-not (Test-Path -LiteralPath $path)) { [System.IO.File]::WriteAllText($path, "[]", $enc) }
    $tmp = "$path.tmp"
    [System.IO.File]::WriteAllText($tmp, $json, $enc)
    # File.Replace 是原子替换，岛仔不会读到半截 JSON；
    # 第三个参数必须显式传 [NullString]::Value，PS 5.1 传 $null 会变成空字符串导致路径非法。
    [System.IO.File]::Replace($tmp, $path, [NullString]::Value)
}

if ($List) {
    ConvertTo-Json -InputObject @(Read-Items) -Depth 6
    return
}
if ($Clear) {
    Write-Items @()
    Write-Host "已清空全部状态"
    return
}
if (-not $Id) { throw "请用 -Id 指定状态 key" }

if ($Remove) {
    $kept = @(Read-Items | Where-Object { (Get-Key $_) -ne $Id })
    Write-Items $kept
    Write-Host "已撤下 $Id"
    return
}
if (-not $Text) { throw "请用 -Text 指定标题" }

$fields = [ordered]@{ id = $Id; text = $Text; detail = $Detail; seconds = $Seconds }
# 边界必须写成 0.0/1.0，否则 Math.Max(int, double) 会把 0.7 取整成 1
if ($Progress -ge 0) { $fields.progress = [Math]::Max(0.0, [Math]::Min(1.0, [double]$Progress)) }
if ($Color) { $fields.color = $Color }
if ($Click) { $fields.click = $Click }
$item = [pscustomobject]$fields

$items = @(Read-Items | Where-Object { (Get-Key $_) -ne $Id })
$items += $item
Write-Items $items
Write-Host "已上岛 / 覆写：$Id"
