$ErrorActionPreference = "Stop"

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$releaseDir = Join-Path $repoRoot "target\release"
$packageBase = Join-Path $repoRoot "target\packages"
$stage = Join-Path $packageBase "pixelforge-windows-x64"
$distDir = Join-Path $repoRoot "dist"
$archive = Join-Path $distDir "pixelforge-windows-x64.zip"

foreach ($required in @("pixelforge-desktop.exe", "pixelforge.exe")) {
    if (-not (Test-Path (Join-Path $releaseDir $required))) {
        throw "Missing $required. Run scripts/build.ps1 first."
    }
}

New-Item -ItemType Directory -Force -Path $packageBase, $distDir | Out-Null
$packageBaseFull = [IO.Path]::GetFullPath($packageBase).TrimEnd('\') + '\'
$stageFull = [IO.Path]::GetFullPath($stage)
if (-not $stageFull.StartsWith($packageBaseFull, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to clean a package path outside target\packages."
}
if (Test-Path $stageFull) { Remove-Item -LiteralPath $stageFull -Recurse -Force }
New-Item -ItemType Directory -Path $stageFull | Out-Null

Copy-Item (Join-Path $releaseDir "pixelforge-desktop.exe") $stageFull
Copy-Item (Join-Path $releaseDir "pixelforge.exe") $stageFull
Copy-Item (Join-Path $repoRoot "README.md") $stageFull
Copy-Item (Join-Path $repoRoot "LICENSE") $stageFull
Copy-Item (Join-Path $repoRoot "docs\FORMATS.md") $stageFull

$redistRoot = $env:VCToolsRedistDir
$vsRoot = Join-Path ([Environment]::GetFolderPath("ProgramFilesX86")) "Microsoft Visual Studio\2022\BuildTools"
if (-not $redistRoot -or -not (Test-Path $redistRoot)) {
    $redistRoot = Join-Path $vsRoot "VC\Redist\MSVC"
}
if (-not (Test-Path $redistRoot)) {
    $vswhere = Join-Path ([Environment]::GetFolderPath("ProgramFilesX86")) "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $vsInstall = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($vsInstall) { $redistRoot = Join-Path $vsInstall "VC\Redist\MSVC" }
    }
}

$crtCandidates = @()
if ($redistRoot -and (Test-Path $redistRoot)) {
    $crtCandidates += Join-Path $redistRoot "x64\Microsoft.VC143.CRT"
    $crtCandidates += Get-ChildItem -LiteralPath $redistRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        ForEach-Object { Join-Path $_.FullName "x64\Microsoft.VC143.CRT" }
}
$crtDirectory = $crtCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $crtDirectory) {
    throw "The Visual C++ x64 runtime files were not found. Install the Visual Studio C++ redistributable component before packaging."
}
Copy-Item (Join-Path $crtDirectory "*.dll") $stageFull

@'
PixelForge for Windows x64

Extract this folder and run pixelforge-desktop.exe. The pixelforge.exe file is the command-line converter.
The bundled Microsoft Visual C++ runtime DLLs keep the app independent of a separately installed VC++ runtime.
Image conversion runs locally and works offline after installation.
'@ | Set-Content -LiteralPath (Join-Path $stageFull "INSTALL.txt") -Encoding utf8

if (Test-Path $archive) {
    $distFull = [IO.Path]::GetFullPath($distDir).TrimEnd('\') + '\'
    $archiveFull = [IO.Path]::GetFullPath($archive)
    if (-not $archiveFull.StartsWith($distFull, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to replace an archive outside dist."
    }
    Remove-Item -LiteralPath $archiveFull -Force
}
Compress-Archive -Path (Join-Path $stageFull "*") -DestinationPath $archive -CompressionLevel Optimal
Write-Host "Portable Windows package: $archive"
