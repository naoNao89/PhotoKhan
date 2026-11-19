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
    [switch]$SkipDumpbin,
    [switch]$SkipStrip
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

    # ------------------------------------------------------------------ strip MLAS AVX2/AVX512
    # Restore pristine tracked files first (git checkout) so every run strips
    # from the original content - idempotent across cache reuses. The checkout
    # ALWAYS runs (even with -SkipStrip) so a baseline build never inherits
    # patched files from a previous run's cache tree.
    & git -C $RepoDir checkout -- cmake/onnxruntime_mlas.cmake onnxruntime/core/mlas/lib/platform.cpp onnxruntime/core/mlas/lib/convsym.cpp onnxruntime/core/mlas/lib/convolve.cpp onnxruntime/core/mlas/lib/reorder.cpp onnxruntime/core/mlas/lib/sconv_nchw_depthwise_multiplier_greater_than_1.cpp onnxruntime/core/mlas/lib/snchwc.h
    if ($LASTEXITCODE -ne 0) { Fail "git checkout of mlas files failed (exit $LASTEXITCODE)." }

    if (-not $SkipStrip) {
    Write-Step "Stripping MLAS AVX2/AVX512 kernel sources (older-CPU support)"
    $mlasCmake = Join-Path $RepoDir 'cmake\onnxruntime_mlas.cmake'
    if (-not (Test-Path $mlasCmake)) { Fail "onnxruntime_mlas.cmake not found at $mlasCmake" }
    $before = Get-Content $mlasCmake -Raw
    $logDir = Split-Path $LogPath -Parent
    Set-Content -Path (Join-Path $logDir 'mlas_cmake.before.txt') -Value $before -Encoding UTF8
    # Also stage copies into the workspace so CI can upload them (D:\build is
    # not accessible from the artifact step).
    $origPath = Join-Path $env:GITHUB_WORKSPACE 'mlas_cmake.before.txt'
    Set-Content -Path $origPath -Value $before -Encoding UTF8
    Write-Host "mlas cmake before: $(Join-Path $logDir 'mlas_cmake.before.txt')"

    # Neutralize AVX2/AVX512/AVX-VNNI kernels:
    # - Lines that are PURE path entries (only ${MLAS_SRC_DIR}/...file.ext) are
    #   deleted - they only ever appear inside list assignments, so the variable
    #   becomes naturally empty and the file structure stays valid.
    # - GLOB patterns (*) are rewritten to __none__ so they expand to nothing.
    # - All other lines (set_property(SOURCE ... openers, PROPERTIES lines,
    #   comments, set(var ...) headers) are kept verbatim: the referenced files
    #   still exist on disk, properties on files not in any target are harmless,
    #   and multi-line statements stay intact.
    # Token matcher uses substring matching (avx512f/avx512_2bit/avx2vnni
    # caught; no \b boundary - _ is a word char and would hide these).
    $purePathRx = '^\s*\$\{MLAS_SRC_DIR\}/[^"()]*\.(cpp|asm|S|inc)\s*$'
    $avxRx = '(?i)(avx2|avx512|avx-512|avxvnni)[\w\-]*'
    $removed = 0
    $after = (($before -split "`r?`n") | ForEach-Object {
        $line = $_
        if ($line -match $avxRx) {
            if ($line -match $purePathRx) {
                $removed++
                return
            }
            if ($line -match '\*') {
                $line = $line -replace $avxRx, '__none__'
            }
        }
        $line
    }) -join "`r`n"

    $removed = ([regex]::Matches($before, "`r?`n")).Count - ([regex]::Matches($after, "`r?`n")).Count
    Write-Host "Stripped $removed line(s) from onnxruntime_mlas.cmake"
    if ($removed -le 0) {
        Set-Content -Path (Join-Path $logDir 'mlas_cmake.after.txt') -Value $after -Encoding UTF8
        Fail "No AVX2/AVX512 source/flag lines were stripped - check cmake/onnxruntime_mlas.cmake layout (see mlas_cmake.before.txt)."
    }
    Set-Content -Path $mlasCmake -Value $after -Encoding UTF8
    Set-Content -Path (Join-Path $logDir 'mlas_cmake.after.txt') -Value $after -Encoding UTF8
    Set-Content -Path (Join-Path $env:GITHUB_WORKSPACE 'mlas_cmake.after.txt') -Value $after -Encoding UTF8
    Write-Host "mlas cmake after: $(Join-Path $logDir 'mlas_cmake.after.txt')"

    # Also stage platform.cpp: its dispatch table references every AVX2/AVX512/
    # AVX-VNNI kernel unconditionally - needed to patch the constructor after the
    # source strip (the link fails on those symbols otherwise).
    $platformCpp = Join-Path $RepoDir 'onnxruntime\core\mlas\lib\platform.cpp'
    if (Test-Path $platformCpp) {
        Copy-Item $platformCpp (Join-Path $env:GITHUB_WORKSPACE 'mlas_platform.cpp') -Force
        Write-Host "mlas platform.cpp staged for the artifact"

        # Stage the whole mlas/lib tree too: other TUs (snchwc.cpp, convolve.cpp,
        # convsym.cpp, ...) reference AVX kernels DIRECTLY, so we need their
        # sources to substitute those references with baseline kernels.
        $mlasLibDir = Join-Path $RepoDir 'onnxruntime\core\mlas\lib'
        if (Test-Path $mlasLibDir) {
            $stageDir = Join-Path $env:GITHUB_WORKSPACE 'mlas_lib_src'
            if (Test-Path $stageDir) { Remove-Item $stageDir -Recurse -Force }
            Copy-Item -Path $mlasLibDir -Destination $stageDir -Recurse -Force
            Write-Host "mlas lib sources staged for the artifact"
        }

        # Patch the AMD64 dispatch constructor by DELETING every assignment line
        # whose RHS references an AVX2/AVX512/AVX-VNNI kernel, plus the
        # "Avx2Supported_ = true;" line. Every such line is a complete single
        # statement, so removing it never breaks syntax; the corresponding slot
        # keeps the baseline SSE/SSE41/AVX value assigned earlier in the
        # constructor. This is idempotent-safe because the file is restored from
        # git below before any patching.
        $platformBefore = Get-Content $platformCpp -Raw
        # Only delete assignment statements (lines starting with "this->") that
        # reference AVX2/AVX512/AVX-VNNI kernels, plus the Avx2Supported_ flag
        # line. NEVER delete other lines (e.g. the MlasOpmask16BitTableAvx512
        # data table's declaration line would break the file - observed C2059).
        $assignmentAvxRx = '^\s*this->.*(?i)(avx2|avx512|avxvnni|avx2vnni|vnni)'
        $avx2FlagRx = '^\s*this->Avx2Supported_\s*=\s*true\s*;'
        $after = (($platformBefore -split "`r?`n") | Where-Object {
            $_ -notmatch $assignmentAvxRx -and $_ -notmatch $avx2FlagRx
        }) -join "`r`n"
        $removedLines = ([regex]::Matches($platformBefore, "`r?`n")).Count - ([regex]::Matches($after, "`r?`n")).Count
        if ($removedLines -le 0) {
            Fail "No AVX references found in platform.cpp - patch did nothing."
        }
        Set-Content -Path $platformCpp -Value $after -Encoding UTF8
        Set-Content -Path (Join-Path $env:GITHUB_WORKSPACE 'mlas_platform.patched.cpp') -Value $after -Encoding UTF8
        Write-Host "platform.cpp patched: removed $removedLines AVX reference line(s)"

        # Other MLAS TUs reference AVX kernels DIRECTLY; patch them too.
        # 1) sconv_nchw_depthwise_multiplier_greater_than_1.cpp: the AMD64
        #    branch asserts the AVX512F kernel and calls it directly. It is dead
        #    after the platform patch (the kernel is never selected), so compile
        #    the #else branch instead by switching the #if to #if 0.
        $sconv = Join-Path $RepoDir 'onnxruntime\core\mlas\lib\sconv_nchw_depthwise_multiplier_greater_than_1.cpp'
        if (Test-Path $sconv) {
            $sconvContent = Get-Content $sconv -Raw
            $marker = '#if defined(MLAS_TARGET_AMD64)'
            if ($sconvContent -notmatch [regex]::Escape($marker)) {
                Fail "sconv AMD64 marker not found"
            }
            $sconvContent = $sconvContent.Replace($marker, '#if 0  // AVX512F kernel stripped; baseline platforms take the #else branch')
            Set-Content -Path $sconv -Value $sconvContent -Encoding UTF8
            Write-Host "sconv_nchw_depthwise_multiplier_greater_than_1.cpp patched (#if 0)"
        }
        # 2) convsym.cpp: the four AVX dispatch struct definitions reference the
        #    stripped kernels, AND the `extern "C" { }` block contains kernel
        #    DEFINITIONS (not declarations) that still need the symbols. Remove
        #    both: the 8 kernel definition lines and the 4 dispatch structs.
        #    Extern declarations in mlasi.h are harmless (declarations only).
        $convsym = Join-Path $RepoDir 'onnxruntime\core\mlas\lib\convsym.cpp'
        if (Test-Path $convsym) {
            $convsymLines = Get-Content $convsym
            # drop kernel definition lines inside the extern "C" block
            $convsymLines = $convsymLines | Where-Object {
                $_ -notmatch '^\s*MLAS_CONV_SYM_(KERNEL|DEPTHWISE_KERNEL) MlasConvSym(Kernel|DepthwiseKernel)Avx(2|Vnni|512)'
            }
            $convsymContent = $convsymLines -join "`r`n"
            $startMarker = 'const MLAS_CONV_SYM_DISPATCH MlasConvSymDispatchAvx2 = {'
            if ($convsymContent -notmatch [regex]::Escape($startMarker)) {
                Fail "convsym Avx2 dispatch marker not found"
            }
            $idx = $convsymContent.IndexOf($startMarker)
            $tail = $convsymContent.Substring($idx)
            $tailIdx = $tail.IndexOf('#endif')
            if ($tailIdx -le 0) { Fail "convsym trailing #endif not found" }
            $convsymContent = $convsymContent.Substring(0, $idx) + $tail.Substring($tailIdx + '#endif'.Length)
            Set-Content -Path $convsym -Value $convsymContent -Encoding UTF8
            Write-Host "convsym.cpp patched: kernel definitions + AVX dispatch structs removed"
        }
        # 3) convolve.cpp: the AVX512F selection condition compares against the
        #    stripped kernel; after the platform patch it can never be true, so
        #    replace the comparison with false (removes the reference).
        $convolve = Join-Path $RepoDir 'onnxruntime\core\mlas\lib\convolve.cpp'
        if (Test-Path $convolve) {
            $syContent = Get-Content $convolve -Raw
            $marker = 'GetMlasPlatform().ConvNchwFloatKernel == MlasConvNchwFloatKernelAvx512F'
            if ($syContent -notmatch [regex]::Escape($marker)) {
                Fail "convolve AVX512F marker not found"
            }
            $syContent = $syContent.Replace($marker, 'false')
            Set-Content -Path $convolve -Value $syContent -Encoding UTF8
            Write-Host "convolve.cpp patched (AVX512F condition -> false)"
        }
        # 4) reorder.cpp: remove the AVX-512 fast-path CALL lines (the enclosing
        #    if (BlockSize==16 ...) blocks stay, become empty, and fall through to
        #    the SSE2 path below on every CPU).
        $reorder = Join-Path $RepoDir 'onnxruntime\core\mlas\lib\reorder.cpp'
        if (Test-Path $reorder) {
            $roLines = Get-Content $reorder
            $final = @()
            $i = 0
            while ($i -lt $roLines.Count) {
                if ($roLines[$i] -match '(?i)MlasReorder(Input|Output)NchwBlock16Avx512F\(') {
                    $i++
                    if ($i -lt $roLines.Count -and $roLines[$i] -match '^\s*continue\s*;\s*$') {
                        $i++
                    }
                    continue
                }
                $final += $roLines[$i]
                $i++
            }
            $roContent = $final -join "`r`n"
            Set-Content -Path $reorder -Value $roContent -Encoding UTF8
            Write-Host "reorder.cpp patched (AVX-512 call lines and their continue removed)"
        }
        # 5) snchwc.h: the sliding-kernel selector compares the platform kernel to
        #    the stripped AVX512F kernels and returns the Sliding kernel. Remove
        #    the WHOLE if-block (condition + return) so no AVX symbols remain and
        #    no `if (false)` C4127 warning appears under /WX; the function just
        #    returns the platform kernel.
        $snchwcH = Join-Path $RepoDir 'onnxruntime\core\mlas\lib\snchwc.h'
        if (Test-Path $snchwcH) {
            $snContent = Get-Content $snchwcH -Raw
            $cond = 'if (PlatformKernel == MlasConvDepthwiseFloatKernelAvx512F &&' + "`r`n" +
                '        (BackendKernelSelectorConfig == nullptr ||' + "`r`n" +
                '         BackendKernelSelectorConfig->nchwc_depthwise_sliding_kernel)) {'
            if ($snContent -notmatch [regex]::Escape($cond)) {
                Fail "snchwc.h AVX512F condition not found"
            }
            # block spans: condition (3 lines) + `return ...Sliding;` + closing `}`
            $block = $cond + "`r`n" +
                '        return MlasConvDepthwiseFloatKernelAvx512FSliding;' + "`r`n" +
                '    }'
            if ($snContent -notmatch [regex]::Escape($block)) {
                Fail "snchwc.h AVX512F block not found"
            }
            $snContent = $snContent.Replace($block, '')
            # The removed block used BackendKernelSelectorConfig, so the parameter
            # is now unreferenced -> C4100 under /WX. Mark it used by inserting
            # (void)BackendKernelSelectorConfig; right after the function's opening
            # brace.
            $snLines = $snContent -split "`r?`n"
            $out = @()
            $inserted = $false
            foreach ($line in $snLines) {
                if (-not $inserted -and $line -match '^\s*\{\s*$' -and $out.Count -gt 0 -and $out[-1] -match '\)') {
                    $out += $line
                    $out += '        (void)BackendKernelSelectorConfig;'
                    $inserted = $true
                } else {
                    $out += $line
                }
            }
            if (-not $inserted) { Fail "snchwc.h: could not locate function body to mark param used" }
            $snContent = $out -join "`r`n"
            Set-Content -Path $snchwcH -Value $snContent -Encoding UTF8
            Write-Host "snchwc.h patched (AVX512F selector block removed, param marked used)"
        }
    } else {
        Write-Host "platform.cpp not found at $platformCpp (informational)"
    }

    } else {
        Write-Host "SkipStrip: building pristine ONNX Runtime (no MLAS AVX2/AVX512 strip)"
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

    # ------------------------------------------------------------- _deps externals
    Write-Step "Verifying _deps externals (re2 / abseil / protobuf / onnx)"
    $depsDir = Join-Path $RepoDir 'build\Windows\Release\_deps'
    if (-not (Test-Path $depsDir)) { Fail "_deps dir not found at $depsDir" }
    $depsSubdirs = Get-ChildItem -Path $depsDir -Directory | Sort-Object Name
    Write-Host "deps subdirs ($($depsSubdirs.Count)):"
    $depsSubdirs | ForEach-Object { Write-Host "  $($_.Name)" }

    Write-Host ""
    # ort-sys 2.0.0-rc.15 static link unconditionally links `re2` (static_link/mod.rs),
    # expecting $ORT_LIB_PATH\_deps\re2-build\Release\re2.lib. ORT 1.31's FetchContent
    # declares re2 with EXCLUDE_FROM_ALL but the MSVC build never produces the lib;
    # the workflow's C2b step bootstraps it. This check is informational only.
    $re2Build = Join-Path $depsDir 're2-build'
    $re2Lib = Join-Path $re2Build 'Release\re2.lib'
    if (Test-Path $re2Lib) {
        Write-Host "OK: $re2Lib ($((Get-Item $re2Lib).Length) bytes)" -ForegroundColor Green
    } else {
        Write-Host "re2.lib missing at $re2Lib (expected - C2b bootstrap will build it)" -ForegroundColor Yellow
        if (Test-Path $re2Build) {
            Write-Host "Contents of $re2Build (recursive *.lib, first 25):"
            Get-ChildItem -Path $re2Build -Recurse -File -Filter '*.lib' | Select-Object -First 25 |
                ForEach-Object { Write-Host "  $($_.FullName)  ($($_.Length) bytes)" }
        }
    }

    # ort-sys also links abseil components from _deps\abseil_cpp-build\absl\<component>\Release
    $abslBase = Join-Path $depsDir 'abseil_cpp-build\absl\base\Release\absl_base.lib'
    if (Test-Path $abslBase) {
        Write-Host "OK: $abslBase"
    } else {
        Write-Host "MISSING: $abslBase (informational - ort-sys needs abseil libs)" -ForegroundColor Yellow
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
