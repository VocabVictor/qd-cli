param(
    [ValidateRange(1, 64)]
    [int]$Jobs = 2
)

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot
$vswhere = 'C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe'
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'

if (-not (Test-Path -LiteralPath $cargo)) {
    throw 'Cargo was not found. Install Rustup first.'
}
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw 'Visual Studio Build Tools was not found.'
}

$vsPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsPath) {
    throw 'The Visual Studio C++ x64 toolchain is missing.'
}

$devCmd = Join-Path $vsPath 'Common7\Tools\VsDevCmd.bat'
$command = 'call "{0}" -arch=amd64 -host_arch=amd64 && set "CARGO_BUILD_JOBS={1}" && "{2}" build --release' -f $devCmd, $Jobs, $cargo
& cmd.exe /d /s /c $command
if ($LASTEXITCODE -ne 0) {
    throw "Cargo build failed with exit code $LASTEXITCODE"
}

$binary = Join-Path $PSScriptRoot 'target\release\qd.exe'
$binDir = Join-Path $PSScriptRoot 'bin'
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
$installedBinary = Join-Path $binDir 'qd.exe'
Copy-Item -LiteralPath $binary -Destination $installedBinary -Force
Get-Item -LiteralPath $installedBinary | Select-Object FullName, Length, LastWriteTime
