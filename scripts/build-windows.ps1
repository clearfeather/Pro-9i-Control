[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$releaseDirectory = Join-Path $projectRoot "release"
$tauriConfigPath = Join-Path $projectRoot "src-tauri\tauri.conf.json"

foreach ($command in @("pnpm", "rustc", "cargo")) {
  if (-not (Get-Command $command -ErrorAction SilentlyContinue)) {
    throw "Required command '$command' was not found. See README.md for Windows build prerequisites."
  }
}

$tauriConfig = Get-Content -LiteralPath $tauriConfigPath -Raw | ConvertFrom-Json
$productName = $tauriConfig.productName
$binaryName = if ($tauriConfig.mainBinaryName) { $tauriConfig.mainBinaryName } else { $productName }
$artifactName = $productName -replace '\s+', '_'
$version = $tauriConfig.version

Push-Location $projectRoot
try {
  pnpm install --frozen-lockfile
  if ($LASTEXITCODE -ne 0) {
    throw "pnpm install failed with exit code $LASTEXITCODE."
  }

  pnpm run tauri build --bundles nsis
  if ($LASTEXITCODE -ne 0) {
    throw "Tauri build failed with exit code $LASTEXITCODE."
  }

  $binaryPath = Join-Path $projectRoot "src-tauri\target\release\$binaryName.exe"
  if (-not (Test-Path -LiteralPath $binaryPath)) {
    throw "Compiled executable was not found at '$binaryPath'."
  }

  $installerDirectory = Join-Path $projectRoot "src-tauri\target\release\bundle\nsis"
  $installer = Get-ChildItem -LiteralPath $installerDirectory -Filter "*-setup.exe" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

  if (-not $installer) {
    throw "NSIS installer was not found in '$installerDirectory'."
  }

  New-Item -ItemType Directory -Path $releaseDirectory -Force | Out-Null

  $architecture = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
    "arm64"
  } else {
    "x64"
  }
  $installerOutput = Join-Path $releaseDirectory "${artifactName}_${version}_${architecture}-setup.exe"
  Copy-Item -LiteralPath $installer.FullName -Destination $installerOutput -Force

  $zipOutput = Join-Path $releaseDirectory "${artifactName}_${version}_windows_${architecture}_portable.zip"
  Compress-Archive -LiteralPath $binaryPath -DestinationPath $zipOutput -Force

  Write-Host ""
  Write-Host "Build complete:" -ForegroundColor Green
  Write-Host "  Installer: $installerOutput"
  Write-Host "  Portable:  $zipOutput"
} finally {
  Pop-Location
}
