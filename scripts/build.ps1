$ErrorActionPreference = "Stop"

$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }

# dav1d-sys includes a source build, but system-deps only uses it when enabled.
$env:SYSTEM_DEPS_DAV1D_BUILD_INTERNAL = "auto"

$pythonRoot = Join-Path $env:APPDATA "Python"
if (Test-Path $pythonRoot) {
    Get-ChildItem -LiteralPath $pythonRoot -Directory -Filter "Python*" -ErrorAction SilentlyContinue |
        ForEach-Object { Join-Path $_.FullName "Scripts" } |
        Where-Object { Test-Path $_ } |
        ForEach-Object { $env:PATH = "$_;$env:PATH" }
}

$pkgConfigBins = Get-ChildItem (Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages") -Directory -Filter "bloodrock.pkg-config-lite_*" -ErrorAction SilentlyContinue |
    ForEach-Object { Get-ChildItem -LiteralPath $_.FullName -Directory -Filter "pkg-config-lite-*" -ErrorAction SilentlyContinue } |
    ForEach-Object { Join-Path $_.FullName "bin" } |
    Where-Object { Test-Path (Join-Path $_ "pkg-config.exe") }
foreach ($pkgConfigBin in $pkgConfigBins) { $env:PATH = "$pkgConfigBin;$env:PATH" }

$nasmPaths = @(
    (Join-Path $PSScriptRoot "..\target\tools\nasm\nasm-3.02"),
    (Join-Path ([Environment]::GetFolderPath("ProgramFiles")) "NASM")
)
foreach ($nasmPath in $nasmPaths) {
    if (Test-Path (Join-Path $nasmPath "nasm.exe")) { $env:PATH = "$nasmPath;$env:PATH" }
}

$vswhere = Join-Path ([Environment]::GetFolderPath("ProgramFilesX86")) "Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $install = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($install) {
        $devCmd = Join-Path $install "Common7\Tools\VsDevCmd.bat"
        if (Test-Path $devCmd) {
            $command = 'call "{0}" -arch=amd64 -host_arch=amd64 && set' -f $devCmd
            $lines = cmd.exe /c $command
            foreach ($line in $lines) {
                if ($line -match '^([^=]+)=(.*)$') {
                    Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
                }
            }
            if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
        }
    }
}

cargo build --release --workspace
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host "Desktop app: target\release\pixelforge-desktop.exe"
Write-Host "CLI:         target\release\pixelforge.exe"

