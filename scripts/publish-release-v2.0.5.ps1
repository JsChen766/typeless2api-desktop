# 发布 GitHub Release v2.0.5win / v2.0.5macos（需本机已 gh auth login）
# 1) 从 Actions 下载 artifact 解压到 release-artifacts/
# 2) 在本仓库根目录: powershell -File scripts/publish-release-v2.0.5.ps1
param(
  [string]$WinSetup = "$PSScriptRoot\..\release-artifacts\typeless-client_2.0.5_x64-setup.exe",
  [string]$MacDmg = "$PSScriptRoot\..\release-artifacts\typeless-client_2.0.5_aarch64.dmg"
)

$ErrorActionPreference = "Stop"
$Repo = "JsChen766/typeless2api-desktop"
$Root = Split-Path $PSScriptRoot -Parent
$Docs = Join-Path $Root "docs"

function Write-Sum($FilePath, $SumFile, $DisplayName) {
  $hash = (Get-FileHash -Path $FilePath -Algorithm SHA256).Hash.ToLower()
  "$hash  $DisplayName" | Set-Content -Path $SumFile -Encoding ascii -NoNewline
  Add-Content -Path $SumFile -Value ""
  return $hash
}

if (-not (Test-Path $WinSetup)) {
  throw "找不到 Windows 安装包: $WinSetup`n请从 Actions artifact typeless-client-v2.0.5-windows-x64 解压 setup.exe 到 release-artifacts/"
}
if (-not (Test-Path $MacDmg)) {
  throw "找不到 macOS DMG: $MacDmg`n请从 Actions artifact typeless-client-v2.0.5-macos-arm64 解压 dmg 到 release-artifacts/"
}

$winName = Split-Path $WinSetup -Leaf
$macName = Split-Path $MacDmg -Leaf
$sumWin = Join-Path $Docs "SHA256SUMS-v2.0.5win.txt"
$sumMac = Join-Path $Docs "SHA256SUMS-v2.0.5macos.txt"

$hWin = Write-Sum $WinSetup $sumWin $winName
$hMac = Write-Sum $MacDmg $sumMac $macName

$tagWin = "v2.0.5win"
$tagMac = "v2.0.5macos"

gh release view $tagWin --repo $Repo 2>$null
if ($LASTEXITCODE -ne 0) {
  gh release create $tagWin --repo $Repo --title $tagWin --notes-file (Join-Path $Docs "release-v2.0.5win.md") --target main
} else {
  gh release edit $tagWin --repo $Repo --notes-file (Join-Path $Docs "release-v2.0.5win.md")
}
gh release upload $tagWin --repo $Repo --clobber $WinSetup $sumWin

gh release view $tagMac --repo $Repo 2>$null
if ($LASTEXITCODE -ne 0) {
  gh release create $tagMac --repo $Repo --title $tagMac --notes-file (Join-Path $Docs "release-v2.0.5macos.md") --target main
} else {
  gh release edit $tagMac --repo $Repo --notes-file (Join-Path $Docs "release-v2.0.5macos.md")
}
gh release upload $tagMac --repo $Repo --clobber $MacDmg $sumMac

Write-Host "Done."
Write-Host "  $tagWin  SHA256 $hWin"
Write-Host "  $tagMac  SHA256 $hMac"
