# Windows build: static ONNX Runtime + NSIS installer

## 1. Purpose & scope

PhotoKhan is a Tauri 2 + Rust desktop app targeting **Windows 10 x64**, CPU-only. The Windows CI build produces a single NSIS installer with ONNX Runtime **statically linked** into `photokhan.exe`, runs inference smoke tests against the bundled YuNet face-detection model, validates the installer by installing it silently and running the installed app, and uploads the artifacts.

Scope:

- Windows 10 x64 runtime (built on `windows-2022` runners).
- CPU-only inference (no GPU execution providers).
- Offline operation: model, manifest and license ship inside the installer; nothing is downloaded at first run.
- ONNX Runtime v1.31.0 built from source as static libraries (baseline CPU, **no AVX/AVX2/AVX512 compile flags**).

## 2. Architecture decisions

- **Static ONNX Runtime v1.31.0, built from source.** `build_ort_baseline.ps1` clones `microsoft/onnxruntime` at tag `v1.31.0` and builds Release static libs with `onnxruntime_BUILD_UNIT_TESTS=OFF`, passing **no** `onnxruntime_USE_AVX*` defines (they default to OFF). After the build it verifies in `CMakeCache.txt` that `onnxruntime_USE_AVX/AVX2/AVX512` are all `=OFF` and refuses to continue otherwise. Reasons:
  - **Older CPU support**: no compile-time AVX flags means the binary runs on CPUs without AVX2/AVX512 (MLAS still dispatches to SIMD at runtime behind CPUID checks).
  - **No DLL shipping**: no `onnxruntime.dll` beside the exe, no version-skew between app and runtime.
  - **ort-sys link contract**: `ort` 2.0.0-rc.15 statically links when `ORT_LIB_PATH` points at the directory containing `Release\` (the `onnxruntime_*.lib` files) and `_deps\` (protobuf/onnx/nsync/cpuinfo/clog build trees), `ORT_LIB_PROFILE=Release`, and `ORT_PREFER_DYNAMIC_LINK=0`. `ORT_SKIP_DOWNLOAD=1` makes a misconfigured build **fail at link time** instead of silently downloading the prebuilt DLL.
- **Model, manifest and license ship as Tauri resources** (`bundle.resources` map form: `{ "../models/": "models/" }` in `tauri.conf.json`). Tauri rewrites parent-traversal in *array* resource entries to `_up_` (`../models/...` would land in `_up_\models\`), so the map form is used to place the files under `models\`. On Windows Tauri v2's `resource_dir` is the exe directory, so the installer ships `manifest.json`, `face_detection_yunet_2023mar.onnx` and `LICENSE` under `<install>\models\`, matching the app's existing `beside_executable()` convention (`<exe>\models`). The app's ModelStore SHA-256 verification is unchanged; the CI asserts the bundled model hash matches `8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4`.
- **WebView2**: `webviewInstallMode = embedBootstrapper` — the installer bootstraps the WebView2 Runtime if missing (WebView2 is preinstalled on `windows-2022` runners; presence is recorded in `report.json` as informational).
- **NSIS `currentUser`**: per-user install, no admin elevation required.

## 3. Local Windows reproduction

Prerequisites:

- Visual Studio 2022 with the **Desktop development with C++** workload (MSVC, vswhere).
- CMake >= 3.28 (ONNX Runtime requires it; `cmake/CMakeLists.txt:6`).
- Python >= 3.8, git, Node 22 + npm.

Steps:

1. Build ONNX Runtime once (30–90 min):

   ```powershell
   powershell -ExecutionPolicy Bypass -File build_ort_baseline.ps1 `
     -BuildRoot D:\build -RepoDir D:\build\onnxruntime `
     -LogPath D:\build\ort_build.log -SkipDumpbin
   ```

   The script prints `ORT_LIB_PATH -> D:\build\onnxruntime\build\Windows\Release` when done.

2. Point ort-sys at the static tree — either set the environment (a `cmd` session / user env):

   ```powershell
   $env:ORT_LIB_PATH = 'D:\build\onnxruntime\build\Windows\Release'
   $env:ORT_LIB_PROFILE = 'Release'
   $env:ORT_PREFER_DYNAMIC_LINK = '0'
   $env:ORT_SKIP_DOWNLOAD = '1'
   ```

   or use a `.cargo/config.toml` `[env]` block mirroring the CI variables (a local example is below; `.cargo/` is git-ignored, so this file stays machine-local):

   ```toml
   [env]
   ORT_LIB_PATH = 'D:\build\onnxruntime\build\Windows\Release'
   ORT_LIB_PROFILE = 'Release'
   ORT_PREFER_DYNAMIC_LINK = '0'
   ORT_SKIP_DOWNLOAD = '1'
   ```

3. Build and test the Rust side:

   ```powershell
   cd src-tauri
   cargo test --lib
   cargo build --release
   ```

4. Build the NSIS installer (from the repo root — `npm` needs the lockfile there):

   ```powershell
   npm ci
   npm run tauri build -- --bundles nsis
   # -> src-tauri\target\release\bundle\nsis\photokhan_0.1.0_x64-setup.exe
   ```

5. Smoke-test inference (dev context):

   ```powershell
   $env:PHOTOKHAN_MODELS_DIR = "$PWD\models"
   .\src-tauri\target\release\photokhan.exe --self-test
   ```

   Exit code 0 and `"status":"ok"` mean the model loaded, its SHA-256 verified, and YuNet ran a synthetic 640×640 inference.

## 4. CI reproduction

- Triggers: `workflow_dispatch`, or push / pull_request against `main`.
- The ONNX Runtime source + build tree at `D:\build\onnxruntime` is cached with key `ort-win-static-1.31.0-<sha256 of build_ort_baseline.ps1>` (restore key `ort-win-static-1.31.0-`). The first run builds ORT from scratch (~30–90 min); later runs restore the cache and the build step is incremental (fast).
- Job env mirrors the local variables: `ORT_LIB_PATH`, `ORT_LIB_PROFILE=Release`, `ORT_PREFER_DYNAMIC_LINK=0`, `ORT_SKIP_DOWNLOAD=1`, `RUST_BACKTRACE=1`.
- All validation steps throw on failure — the job fails on any missing required artifact; the installer is uploaded only when every assertion passed.

## 5. Verification evidence

| Claim | Status |
|---|---|
| Build on windows-2022 runner | VERIFIED-BY-CI-RUN |
| Inference on runner (dev context) | VERIFIED-BY-CI-RUN |
| Installer + installed-run inference | VERIFIED-BY-CI-RUN |
| Windows 10 runtime | UNVERIFIED (needs real Win10 hardware) |
| Older CPU / no-AVX2 | UNVERIFIED (baseline ORT build has no compile-time AVX flags; runtime-dispatch + real-hardware test still required) |
| WebView2 on Windows 10 | UNVERIFIED-on-target |

Update the status column after a CI run. The `windows-2022` runner evidence (build, dev-context inference, silent install + installed-run inference) comes from the `photokhan-windows-x64` artifact of the run; Windows 10 and no-AVX2 claims need a real Windows 10 machine without AVX2.

## 6. Artifacts

Uploaded by `windows-build.yml` (artifact `photokhan-windows-x64` on success; `windows-build-diagnostics` always):

- `src-tauri/target/release/bundle/nsis/photokhan_0.1.0_x64-setup.exe` — the NSIS installer (never uploaded when validation failed).
- `report.json` — single JSON object: git ref/sha, run id, ORT strategy (`static`, v1.31.0), dumpbin summary (no `onnxruntime.dll`; `vcruntime140.dll` / `msvcp140.dll` / `api-ms-win-*` entries), Tauri resource layout actually produced (`models` expected; fallbacks `resources\models`, `exe_dir`, `resources`), installed-dir file list, dev + installed self-test results (status, resolution, model hash match), installer/exe/model SHA-256, WebView2 presence.
- `dumpbin.txt` — `/dependents` output for the built `photokhan.exe`.
- `hashes.txt` — SHA-256 of installer, exe and model.
- `selftest-dev.json`, `installed-selftest.json` — the two self-test JSON documents.
- `ort_build.log` (diagnostics only) — the ORT build transcript from `D:\build`.

## 7. Licenses

- **YuNet** face detection model — MIT, Shiqi Yu; the model's license text ships as `models/LICENSE` inside the installer.
- **ONNX Runtime v1.31.0** — MIT; statically linked, no runtime files distributed.
- **WebView2 Runtime** — Microsoft runtime, redistributable per Microsoft terms; the installer bootstraps it when absent.

See `THIRD_PARTY_LICENSES.md` for the project-level license list.

## 8. Troubleshooting

- **Cache key invalidation**: the cache key embeds the hash of `build_ort_baseline.ps1` — edit that file to force a new key. The restore key (`ort-win-static-1.31.0-`) still pulls the previous tree, so the rebuild starts from the cached checkout/build and is incremental rather than from scratch.
- **`ORT_LIB_PATH` shape**: it must be the directory containing `Release\` **and** `_deps\` — i.e. `D:\build\onnxruntime\build\Windows\Release`, *not* `...\Release\Release` (that's the profile subdir holding the `.lib` files). ort-sys looks up `%ORT_LIB_PATH%\Release\*.lib` plus the externals under `%ORT_LIB_PATH%\_deps\`.
- **`ORT_SKIP_DOWNLOAD` semantics**: `1` makes ort-sys fail the build if the static tree is missing/misconfigured, instead of falling back to downloading the prebuilt `onnxruntime.dll`. Leave it set in CI; the "C3 guard" step additionally throws if `onnxruntime_common.lib` or `_deps\protobuf-build` are absent.
- **dumpbin check**: `dumpbin /dependents photokhan.exe` must **not** list `onnxruntime.dll` (that would mean dynamic linking). `vcruntime140.dll`, `msvcp140.dll` and `api-ms-win-*` entries are expected and fine — they come from the MSVC toolchain.
- **Silent-install `C:\phk-ci-install`**: the CI wipes the directory before installing; the installed-run self-test runs from a fresh temp cwd with `PHOTOKHAN_MODELS_DIR` unset and asserts `resolution.selected` is exe-relative (`exe_dir`, `exe_dir/resources` or `exe_dir/models`), proving the installed app finds and verifies its model with no cwd/source-tree dependence. Model files are expected under `<install>\models\`.
