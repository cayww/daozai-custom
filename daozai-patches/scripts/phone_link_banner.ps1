<#
.SYNOPSIS
检查 / 补齐手机连接（Phone Link）通知通道的 ShowBanner=0。

新装手机 App 会新建 AUMID 通道，默认会弹右下角横幅；本脚本按
Microsoft.YourPhone* 前缀补漏。只改 ShowBanner，绝不动 Enabled / ShowInActionCenter，
否则通知不产生或不进通知中心，岛仔就收不到了。

.EXAMPLE
.\phone_link_banner.ps1             # 只读体检
.\phone_link_banner.ps1 -Fix        # 把缺失/非 0 的通道补成 0
#>
param([switch]$Fix)

$ErrorActionPreference = "Stop"
$base = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings"
if (-not (Test-Path -LiteralPath $base)) { throw "找不到通知设置注册表项：$base" }

$keys = @(Get-ChildItem -LiteralPath $base | Where-Object { $_.PSChildName -like "Microsoft.YourPhone*" })
$report = foreach ($key in $keys) {
    $value = (Get-ItemProperty -LiteralPath $key.PSPath -Name ShowBanner -ErrorAction SilentlyContinue).ShowBanner
    [pscustomobject]@{
        Channel    = $key.PSChildName
        ShowBanner = if ($null -eq $value) { "<未设置>" } else { $value }
    }
}

$report | Format-Table -AutoSize
$bad = @($report | Where-Object { $_.ShowBanner -ne 0 })
Write-Host "Phone Link 通道总数：$($report.Count)；需要补设：$($bad.Count)"

if (-not $Fix) {
    Write-Host "只读模式：加 -Fix 才会写入注册表"
    return
}
foreach ($key in $keys) {
    New-ItemProperty -LiteralPath $key.PSPath -Name ShowBanner -Value 0 -PropertyType DWord -Force | Out-Null
}
Write-Host "已把 $($keys.Count) 个通道的 ShowBanner 设为 0（Enabled / ShowInActionCenter 未改动）"
