$ErrorActionPreference = "Stop"
& (Join-Path $PSScriptRoot "build.ps1")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
& (Join-Path $PSScriptRoot "..\target\release\pixelforge-desktop.exe")

