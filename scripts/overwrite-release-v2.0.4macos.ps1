# 用本地 DMG 覆盖 GitHub Release v2.0.4macos（在 Windows 上运行，需已安装 gh）。
param(
  [string]$DmgPath = "$PSScriptRoot\..\release-artifacts\typeless-client_2.0.4_aarch64.dmg"
)

$ErrorActionPreference = "Stop"
$Repo = "JsChen766/typeless2api-desktop"
$Tag = "v2.0.4macos"
$DmgName = "typeless-client_2.0.4_aarch64.dmg"
$SumName = "SHA256SUMS-v2.0.4macos.txt"

if (-not (Test-Path $DmgPath)) {
  throw "找不到 DMG: $DmgPath （请先在 Mac 上运行 scripts/build-macos-dmg.sh 并把 DMG 放到 release-artifacts/）"
}

$root = Split-Path $PSScriptRoot -Parent
$sumPath = Join-Path $root "docs\$SumName"
$hash = (Get-FileHash -Path $DmgPath -Algorithm SHA256).Hash.ToLower()
"$hash  $DmgName" | Set-Content -Path $sumPath -Encoding ascii -NoNewline
Add-Content -Path $sumPath -Value ""

gh release delete-asset $Tag $DmgName --repo $Repo -y 2>$null
gh release delete-asset $Tag $SumName --repo $Repo -y 2>$null
gh release upload $Tag --repo $Repo --clobber $DmgPath $sumPath

Write-Host "Uploaded $Tag"
Write-Host "SHA256: $hash"
