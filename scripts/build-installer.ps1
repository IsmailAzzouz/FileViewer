<#
.SYNOPSIS
    Builds FileViewer release binary and packages the Windows installer.
.DESCRIPTION
    1. Generates icons and installer bitmaps if not already present.
    2. Builds the release binary with cargo.
    3. Finds the Inno Setup compiler (ISCC.exe).
    4. Compiles the Inno Setup script into dist/FileViewer-Setup-<version>.exe.
    5. Computes and displays file size and SHA256 checksum.
#>

[CmdletBinding()]
param (
    [switch]$SkipBuild,
    [string]$OutputDir = "dist"
)

$ErrorActionPreference = "Stop"
$ProjectRoot = (Get-Item $PSScriptRoot).Parent.FullName

Write-Host "====================================================" -ForegroundColor Cyan
Write-Host "   FileViewer - Windows Installer Builder           " -ForegroundColor Cyan
Write-Host "====================================================" -ForegroundColor Cyan

Set-Location $ProjectRoot

# 1. Ensure assets exist
$iconPath = Join-Path $ProjectRoot "assets\icon.ico"
$wizardLarge = Join-Path $ProjectRoot "assets\installer\wizard-large.bmp"
if (-not (Test-Path $iconPath) -or -not (Test-Path $wizardLarge)) {
    Write-Host "[1/4] Generating icon and installer image assets..." -ForegroundColor Yellow
    python (Join-Path $ProjectRoot "scripts\generate_icons.py")
} else {
    Write-Host "[1/4] Asset icons already generated." -ForegroundColor Green
}

# 2. Build Release Binary
$releaseExe = Join-Path $ProjectRoot "target\release\file-viewer.exe"
if (-not $SkipBuild -or -not (Test-Path $releaseExe)) {
    Write-Host "[2/4] Building release binary with Cargo..." -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Cargo release build failed."
        exit 1
    }
} else {
    Write-Host "[2/4] Skipping cargo build (-SkipBuild specified)." -ForegroundColor Green
}

if (-not (Test-Path $releaseExe)) {
    Write-Error "Release executable not found at: $releaseExe"
    exit 1
}

# 3. Locate Inno Setup Compiler (ISCC.exe)
Write-Host "[3/4] Locating Inno Setup 6 compiler..." -ForegroundColor Yellow
$isccCandidates = @(
    "ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
)

$isccPath = $null
foreach ($candidate in $isccCandidates) {
    if (Get-Command $candidate -ErrorAction SilentlyContinue) {
        $isccPath = (Get-Command $candidate).Source
        break
    }
    if (Test-Path $candidate) {
        $isccPath = (Get-Item $candidate).FullName
        break
    }
}

if (-not $isccPath) {
    Write-Error "Inno Setup compiler (ISCC.exe) could not be found. Please ensure Inno Setup 6 is installed."
    exit 1
}
Write-Host "Found Inno Setup at: $isccPath" -ForegroundColor Green

# 4. Compile Installer
Write-Host "[4/4] Compiling installer with Inno Setup..." -ForegroundColor Yellow
$issPath = Join-Path $ProjectRoot "installer\FileViewer.iss"

$distPath = Join-Path $ProjectRoot $OutputDir
if (-not (Test-Path $distPath)) {
    New-Item -ItemType Directory -Path $distPath | Out-Null
}

& $isccPath "/O$distPath" $issPath
if ($LASTEXITCODE -ne 0) {
    Write-Error "Installer compilation failed."
    exit 1
}

# 5. Create Portable ZIP Archive
Write-Host "[5/5] Creating portable ZIP archive..." -ForegroundColor Yellow
$zipName = "FileViewer-0.1.0-windows-x86_64.zip"
$zipPath = Join-Path $distPath $zipName
if (Test-Path $zipPath) { Remove-Item -Force $zipPath }

$tempStage = Join-Path $distPath "temp_zip_stage"
if (Test-Path $tempStage) { Remove-Item -Recurse -Force $tempStage }
New-Item -ItemType Directory -Path $tempStage | Out-Null

Copy-Item $releaseExe -Destination $tempStage
Copy-Item (Join-Path $ProjectRoot "README.md") -Destination $tempStage
Copy-Item (Join-Path $ProjectRoot "LICENSE") -Destination $tempStage
New-Item -ItemType Directory -Path (Join-Path $tempStage "assets") | Out-Null
Copy-Item (Join-Path $ProjectRoot "assets\icon.ico") -Destination (Join-Path $tempStage "assets")
Copy-Item (Join-Path $ProjectRoot "samples") -Destination $tempStage -Recurse

Compress-Archive -Path "$tempStage\*" -DestinationPath $zipPath -CompressionLevel Optimal
Remove-Item -Recurse -Force $tempStage

# Locate output files
$installerFile = Get-ChildItem -Path $distPath -Filter "FileViewer-Setup-*.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$zipFile = Get-Item $zipPath

Write-Host ""
Write-Host "====================================================" -ForegroundColor Green
Write-Host "  Windows Artifacts Generated Successfully!         " -ForegroundColor Green
Write-Host "====================================================" -ForegroundColor Green
if ($installerFile) {
    $sha256 = (Get-FileHash -Path $installerFile.FullName -Algorithm SHA256).Hash
    $sizeMB = [math]::Round($installerFile.Length / 1MB, 2)
    Write-Host "Installer: $($installerFile.FullName)" -ForegroundColor White
    Write-Host "Size:      $($installerFile.Length) bytes ($sizeMB MB)" -ForegroundColor White
    Write-Host "SHA256:    $sha256" -ForegroundColor White
}
if ($zipFile) {
    $zipSha256 = (Get-FileHash -Path $zipFile.FullName -Algorithm SHA256).Hash
    $zipSizeMB = [math]::Round($zipFile.Length / 1MB, 2)
    Write-Host ""
    Write-Host "Portable:  $($zipFile.FullName)" -ForegroundColor White
    Write-Host "Size:      $($zipFile.Length) bytes ($zipSizeMB MB)" -ForegroundColor White
    Write-Host "SHA256:    $zipSha256" -ForegroundColor White
}
# Update dist/SHA256SUMS if present
$sumsFile = Join-Path $distPath "SHA256SUMS"
if (Test-Path $sumsFile) {
    $lines = Get-Content $sumsFile
    $newLines = @()
    foreach ($line in $lines) {
        if ($line -match "FileViewer-Setup-.*\.exe") {
            $newLines += "$($sha256.ToLower())  ./$($installerFile.Name)"
        } elseif ($line -match "FileViewer-.*windows-x86_64\.zip") {
            $newLines += "$($zipSha256.ToLower())  ./$($zipFile.Name)"
        } else {
            $newLines += $line
        }
    }
    Set-Content -Path $sumsFile -Value $newLines -NoNewline:$false
}

Write-Host "====================================================" -ForegroundColor Green
