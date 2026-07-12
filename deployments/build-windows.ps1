#!/usr/bin/env pwsh
$ErrorActionPreference = "Stop"

$AppName = "WebSocketReflectorX"
$AppRoot = "dist"
$DistDir = $AppName
$PortableName = "$AppName-portable-windows-msvc-x86_64.zip"
$InstallerName = "$AppName-installer-windows-msvc-x86_64.exe"

function Test-Command($cmd) {
    return [bool](Get-Command $cmd -ErrorAction SilentlyContinue)
}

# Validate working directory
if ((Split-Path -Leaf (Get-Location)) -ne $AppName) {
    Write-Error "This script must be run from the $AppName directory."
    exit 1
}

# Validate toolchain dependencies
if (-not (Test-Command cargo)) {
    Write-Error "cargo not found. Please install Rust and add it to PATH."
    exit 1
}

if (-not (Test-Command 7z)) {
    Write-Error "7z not found. Please install 7-Zip and add it to PATH."
    exit 1
}

if (-not (Test-Command makensis)) {
    Write-Error "makensis not found. Please install NSIS and add it to PATH."
    exit 1
}

# Clean previous build artifacts
$pathsToClean = @($AppRoot, $DistDir, "windows/$DistDir", $PortableName, $InstallerName)
foreach ($p in $pathsToClean) {
    if (Test-Path $p) {
        Remove-Item -Recurse -Force $p
    }
}

# Build release binaries
Write-Host "---- Building release binaries"
cargo build --release --bins

# Prepare distribution files
Write-Host "---- Preparing distribution files"
New-Item -ItemType Directory -Force -Path $AppRoot | Out-Null
Copy-Item "target/release/wsrx.exe" "$AppRoot/wsrx.exe"
Copy-Item "target/release/wsrx-desktop.exe" "$AppRoot/wsrx-desktop.exe"

# Optional: include Visual C++ Redistributable installer so setup.nsi can run it.
# Download from: https://aka.ms/vs/17/release/vc_redist.x64.exe
if (Test-Path "windows/vc_redist.x64.exe") {
    Copy-Item "windows/vc_redist.x64.exe" "$AppRoot/vc_redist.x64.exe"
} else {
    Write-Warning "windows/vc_redist.x64.exe not found. The NSIS installer will fail to install the VC++ redistributable. You can download it from https://aka.ms/vs/17/release/vc_redist.x64.exe"
}

Rename-Item $AppRoot $DistDir

# Create portable package
Write-Host "---- Creating portable package"
7z a $PortableName $DistDir

# Create installer
Write-Host "---- Creating installer"
Move-Item $DistDir "windows/$DistDir"
Copy-Item "windows/$AppName.ico" "windows/$DistDir/$AppName.ico"
makensis "windows/setup.nsi"

# Move installer to project root
Get-ChildItem "windows/*.exe" | Move-Item -Destination "./$InstallerName"

Write-Host "---- Done"
Write-Host "Portable package: $PortableName"
Write-Host "Installer:        $InstallerName"
