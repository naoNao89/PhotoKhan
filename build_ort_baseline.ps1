#Requires -Version 5.1
<#
    build_ort_baseline.ps1
    Build ONNX Runtime v1.31.0 (static, Release) without enabling any SIMD flag.

    Verified facts this script relies on (tag v1.31.0):
      - cmake/CMakeLists.txt:6                  -> cmake_minimum_required(VERSION 3.28)
      - cmake/CMakeLists.txt:145-147            -> onnxruntime_USE_AVX / AVX2 / AVX512, default OFF
      - cmake/adjust_global_compile_flags.cmake:253-262 -> /arch:* only added when those are ON
      - cmake/CMakeLists.txt:149                -> onnxruntime_BUILD_SHARED_LIB default OFF (static)
      - cmake/CMakeLists.txt:94                 -> onnxruntime_BUILD_UNIT_TESTS default ON (we turn tests off)
      - tools/ci_build/build_args.py:189-190    -> supported VS generators
      - tools/ci_build/build.py:67-75           -> Python >= 3.8
    No SIMD flag is passed: the build stays at the compiler default baseline.
#>
[CmdletBinding()]
param(
    [string]$BuildRoot = 'D:\build',
    [string]$RepoDir   = 'D:\build\onnxruntime',
    [string]$Tag       = 'v1.31.0',
    [string]$LogPath   = 'D:\build\ort_build.log',
    [switch]$SkipDumpbin
)

$ErrorActionPreference = 'Stop'

function Write-Step {
    param([string]$Message)
    Write-Host ""
    Write-Host "=== $Message ===" -ForegroundColor Cyan
}

function Fail {
    param([string]$Message)
    Write-Host ""
    Write-Host "ERROR: $Message" -ForegroundColor Red
    exit 1
}

function Import-VsEnvironment {
    param([string]$VsInstallPath)

    $vcvars = Join-Path $VsInstallPath 'VC\Auxiliary\Build\vcvars64.bat'
    if (-not (Test-Path $vcvars)) {
        Fail "vcvars64.bat not found under '$VsInstallPath'. C++ workload (MSVC) is probably not installed."
    }
    Write-Host "Importing MSVC environment from: $vcvars"
    $lines = & cmd.exe /d /c "call `"$vcvars`" >nul 2>&1 && set"
    if ($LASTEXITCODE -ne 0) { Fail "vcvars64.bat failed (exit $LASTEXITCODE)." }
    foreach ($line in $lines) {
        if ($line -match '^([^=]+)=(.*)$') {
            Set-Item -Path ("Env:" + $matches[1]) -Value $matches[2]
        }
    }
}

New-Item -ItemType Directory -Force -Path $BuildRoot | Out-Null
try { Start-Transcript -Path $LogPath -Force | Out-Null } catch { Write-Host "WARN: cannot start transcript: $($_.Exception.Message)" }

try {
    # ------------------------------------------------------------------ tools
    Write-Step "Checking required tools"
    $missing = @()

    # cmake >= 3.28 (cmake/CMakeLists.txt:6)
    $cmakeCmd = Get-Command cmake.exe -ErrorAction SilentlyContinue
    $cmakeVersion = $null
    if (-not $cmakeCmd) {
        $missing += 'cmake (>= 3.28) not found on PATH'
    } else {
        $raw = (& cmake --version | Select-Object -First 1)
        $cmakeVersion = ($raw -replace '^cmake version\s+', '').Trim()
        Write-Host "cmake   : $($cmakeCmd.Source)  ($cmakeVersion)"
        $numeric = ($cmakeVersion -replace '^([0-9]+(\.[0-9]+){1,3}).*$', '$1')
        if ($numeric -and ([version]$numeric -lt [version]'3.28.0')) {
            $missing += "cmake >= 3.28 (found $cmakeVersion)"
        }
    }

    # python >= 3.8 (tools/ci_build/build.py:67-75)
    $pythonCmd = Get-Command python.exe -ErrorAction SilentlyContinue
    if (-not $pythonCmd) {
        $missing += 'python (>= 3.8) not found on PATH'
    } else {
        $pyRaw = (& python --version 2>&1)
        Write-Host "python  : $($pythonCmd.Source)  ($pyRaw)"
        if ($pyRaw -match 'Python\s+(\d+)\.(\d+)\.') {
            if (([int]$matches[1] -lt 3) -or (([int]$matches[1] -eq 3) -and ([int]$matches[2] -lt 8))) {
                $missing += "python >= 3.8 (found $pyRaw)"
            }
        }
    }

    # git
    $gitCmd = Get-Command git.exe -ErrorAction SilentlyContinue
    if (-not $gitCmd) {
        $missing += 'git not found on PATH'
    } else {
        Write-Host "git     : $($gitCmd.Source)  ($(& git --version))"
    }

    # MSVC via vswhere, prefer VS 2022 (17.x) because that is ORT's default generator
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $generator = $null
    $vsPath = $null
    if (-not (Test-Path $vswhere)) {
        $missing += 'vswhere.exe (Visual Studio Installer) not found - MSVC Build Tools missing?'
    } else {
        $vsPath = & $vswhere -latest -products * -version '[17.0,18.0)' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($vsPath) {
            $generator = 'Visual Studio 17 2022'
        } else {
            $vsPath = & $vswhere -latest -products * -version '[18.0,19.0)' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
            if ($vsPath) { $generator = 'Visual Studio 18 2026' }
        }
        if (-not $vsPath) {
            $missing += 'MSVC C++ build tools (VS 2022 or VS 18 with VC.Tools.x86.x64) not found'
        } else {
            Write-Host "MSVC    : $vsPath  (generator: $generator)"
            Import-VsEnvironment -VsInstallPath $vsPath
            $cl = Get-Command cl.exe -ErrorAction SilentlyContinue
            if (-not $cl) { $missing += 'cl.exe not available after loading vcvars64.bat' }
            else { Write-Host "cl      : $($cl.Source)" }
        }
    }

    if ($missing.Count -gt 0) {
        Write-Host ""
        Write-Host "Missing prerequisites:" -ForegroundColor Yellow
        $missing | ForEach-Object { Write-Host "  - $_" -ForegroundColor Yellow }
        Write-Host ""
        Write-Host "Suggested installs (run manually, then re-run this script):"
        Write-Host "  winget install --id Kitware.CMake -e"
        Write-Host "  winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override `"--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`""
        Write-Host "  winget install --id Git.Git -e"
        Write-Host "  winget install --id Python.Python.3.12 -e"
        Fail 'Prerequisites missing - build not started.'
    }

    # ------------------------------------------------------------------ clone
    Write-Step "Cloning ONNX Runtime $Tag into $RepoDir"
    $gitDir = Join-Path $RepoDir '.git'
    if (Test-Path $gitDir) {
        $currentTag = (& git -C $RepoDir describe --tags --exact-match 2>$null)
        if ($currentTag -ne $Tag) {
            Fail "Existing clone at '$RepoDir' is at '$currentTag', expected '$Tag'. Move it away or delete it manually."
        }
        Write-Host "Reusing existing clone at $Tag"
    } else {
        if (Test-Path $RepoDir) {
            Fail "Directory '$RepoDir' exists but is not a git clone. Move it away or delete it manually."
        }
        & git clone --depth 1 --branch $Tag --recurse-submodules --shallow-submodules https://github.com/microsoft/onnxruntime.git $RepoDir
        if ($LASTEXITCODE -ne 0) { Fail "git clone failed (exit $LASTEXITCODE)." }
    }

    # ------------------------------------------------------------------ build
    Write-Step "Building static Release (no SIMD flags)"
    $buildBat = Join-Path $RepoDir 'build.bat'
    if (-not (Test-Path $buildBat)) { Fail "build.bat not found at $buildBat" }

    # Only non-SIMD options:
    #   --skip_tests                         : do not run ORT unit tests after build
    #   --cmake_extra_defines onnxruntime_BUILD_UNIT_TESTS=OFF (cmake/CMakeLists.txt:94)
    #   --cmake_generator ...                : tools/ci_build/build_args.py:189-190
    # No onnxruntime_USE_AVX* define is passed; they stay OFF by default.
    $argLine = "--config Release --parallel --skip_tests --cmake_generator `"$generator`" --cmake_extra_defines onnxruntime_BUILD_UNIT_TESTS=OFF"
    Write-Host "Running: `"$buildBat`" $argLine"
    Write-Host "This takes a long time (30-90 min). Log: $LogPath"
    Push-Location $RepoDir
    & cmd.exe /d /c "call `"$buildBat`" $argLine"
    $buildExit = $LASTEXITCODE
    Pop-Location
    if ($buildExit -ne 0) { Fail "build.bat failed with exit code $buildExit. See log: $LogPath" }

    # ------------------------------------------------------ verify CMakeCache
    Write-Step "Verifying SIMD options in CMakeCache.txt"
    $cachePath = Join-Path $RepoDir 'build\Windows\Release\CMakeCache.txt'
    if (-not (Test-Path $cachePath)) { Fail "CMakeCache.txt not found at $cachePath" }
    $avxLines = Select-String -Path $cachePath -Pattern '^onnxruntime_USE_AVX(2|512)?:BOOL='
    if (-not $avxLines) { Fail 'No onnxruntime_USE_AVX* entries found in CMakeCache.txt' }
    foreach ($line in $avxLines) { Write-Host "  $($line.Line)" }
    $notOff = $avxLines | Where-Object { $_.Line -notmatch '=OFF$' }
    if ($notOff) {
        $notOff | ForEach-Object { Write-Host "  BAD: $($_.Line)" -ForegroundColor Red }
        Fail 'SIMD options are not OFF - refusing to continue.'
    }
    Write-Host "OK: all onnxruntime_USE_AVX* options are OFF" -ForegroundColor Green

    # ------------------------------------------------------------- artifacts
    Write-Step "Build artifacts"
    $mlas = Get-ChildItem -Path $RepoDir -Recurse -File -Filter 'onnxruntime_mlas.lib' |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if (-not $mlas) { Fail 'onnxruntime_mlas.lib not found after build.' }

    $libDir = $mlas.Directory.FullName
    $libs = Get-ChildItem -Path $libDir -File -Filter '*.lib' | Sort-Object Name
    Write-Host "Library directory: $libDir"
    Write-Host "Library count    : $($libs.Count)"
    $libs | ForEach-Object { Write-Host ("  {0,-45} {1,12:N0} bytes" -f $_.Name, $_.Length) }

    # ORT_LIB_PATH must point at the directory that contains 'Release\' + '_deps\'
    # (ort-sys static_link config #1, build/static_link/mod.rs:125)
    $ortLibPath = $libDir
    if ((Split-Path $libDir -Leaf) -eq 'Release') { $ortLibPath = Split-Path $libDir -Parent }
    Write-Host ""
    Write-Host "ORT_LIB_PATH  -> $ortLibPath"
    Write-Host "ORT_LIB_PROFILE -> Release"

    Write-Step "SHA-256 of onnxruntime_mlas.lib"
    $sha = (Get-FileHash -Path $mlas.FullName -Algorithm SHA256).Hash
    Write-Host "  $sha  $($mlas.FullName)"

    # ------------------------------------------------------------- dumpbin
    Write-Step "dumpbin: SHLX count in onnxruntime_mlas.lib (informational only)"
    if ($SkipDumpbin) {
        Write-Host "Skipped (-SkipDumpbin)."
    } else {
        $dumpbin = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
        if (-not $dumpbin) {
            $dumpbin = Get-ChildItem -Path (Join-Path $vsPath 'VC\Tools\MSVC') -Recurse -File -Filter 'dumpbin.exe' -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match 'Hostx64\\x64' } | Select-Object -First 1
        }
        if (-not $dumpbin) {
            Write-Host "dumpbin.exe not found - run this inside a VS Developer PowerShell to get the count."
        } else {
            $dumpbinExe = if ($dumpbin.Source) { $dumpbin.Source } else { $dumpbin.FullName }
            Write-Host "Using: $dumpbinExe"
            $shlx = 0
            & $dumpbinExe /nologo /disasm:nobytes $mlas.FullName 2>$null | ForEach-Object {
                if ($_ -imatch '\bSHLX\b') { $shlx += ([regex]::Matches($_, '(?i)\bSHLX\b')).Count }
            }
            Write-Host "SHLX occurrences: $shlx"
            Write-Host "NOTE: non-zero is expected - MLAS AVX2 objects are always compiled with /arch:AVX2"
            Write-Host "      (cmake/onnxruntime_mlas.cmake:259) and are only called behind the CPUID check"
            Write-Host "      (onnxruntime/core/mlas/lib/platform.cpp:546). This is NOT a pass/fail criterion."
        }
    }

    Write-Step "Done"
    Write-Host "Build log : $LogPath"
    Write-Host "Next: Bước 3 - keep .cargo\config.toml pointing at ORT_LIB_PATH above, then run:"
    Write-Host "  cd src-tauri"
    Write-Host "  cargo test --lib -- --nocapture"
    exit 0
}
finally {
    try { Stop-Transcript | Out-Null } catch { }
}
