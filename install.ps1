$ErrorActionPreference = 'Stop'

# RoninKB installer for Windows (PowerShell).
# Usage: irm https://raw.githubusercontent.com/kidneyweakx/RoninKB/main/install.ps1 | iex

$Repo = "kidneyweakx/RoninKB"
$InstallDir = "$env:LOCALAPPDATA\RoninKB"

Write-Host "Fetching latest RoninKB release..."
$Latest = (Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest").tag_name
if (-not $Latest) {
    Write-Host "Failed to fetch latest release. Falling back to v0.1.0"
    $Latest = "v0.1.0"
}

$AssetName = "roninKB-$Latest-x86_64-pc-windows-msvc.zip"
$Url = "https://github.com/$Repo/releases/download/$Latest/$AssetName"

$Tmp = New-TemporaryFile
$ZipPath = "$($Tmp.FullName).zip"
Move-Item $Tmp.FullName $ZipPath

Write-Host "Downloading $Url"
Invoke-WebRequest -Uri $Url -OutFile $ZipPath

if (Test-Path $InstallDir) {
    Remove-Item -Recurse -Force $InstallDir
}
New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

$Extract = Join-Path $env:TEMP "roninkb-extract-$([guid]::NewGuid())"
New-Item -ItemType Directory -Path $Extract -Force | Out-Null
Expand-Archive -Path $ZipPath -DestinationPath $Extract -Force

$Root = Get-ChildItem -Path $Extract -Directory | Select-Object -First 1
Copy-Item -Recurse -Force -Path (Join-Path $Root.FullName "*") -Destination $InstallDir

Remove-Item $ZipPath -Force
Remove-Item -Recurse -Force $Extract

Write-Host ""
Write-Host "Installed to $InstallDir"
Write-Host ""
Write-Host "Add to PATH (run as Administrator, one-time):"
Write-Host "  setx PATH `"`$env:PATH;$InstallDir\bin`""
Write-Host ""
Write-Host "Then start the daemon:"
Write-Host "  hhkb-daemon"
Write-Host "And open http://127.0.0.1:7331/"
