# Windows build: static ONNX Runtime inference DLL (`phk_dll.dll`)

## 1. Purpose & scope

PhotoKhan's Windows CI builds one standalone x64 inference DLL, `phk_dll.dll`, from the `dll/` crate (package `phk-dll`). The pipeline ships **no Tauri app and no NSIS installer** — the deliverable is a self-contained DLL bundle (section 4).

The DLL's contract: **input photo → ID-photo JPG**, preset 3×4 (354×472) — crop/alignment with white padding for the area outside the original image. **No background removal** (a later milestone). Face detection uses the YuNet model (`face_detection_yunet_2023mar.onnx`); inference runs on **statically linked ONNX Runtime**.

Scope:

- Windows 10 x64 runtime (built on `windows-2022` runners).
- CPU-only inference (no GPU execution providers).
- Offline operation: model, manifest and licenses ship inside the bundle; nothing is downloaded at runtime.
- ONNX Runtime v1.31.0 built from source as static libraries (baseline CPU, **no AVX/AVX2/AVX512 compile flags**).

## 2. Architecture

- **Static ONNX Runtime v1.31.0, built from source.** `build_ort_baseline.ps1` clones `microsoft/onnxruntime` at tag `v1.31.0` and builds Release static libs with `onnxruntime_BUILD_UNIT_TESTS=OFF`, passing **no** `onnxruntime_USE_AVX*` defines (they default to OFF). After the build it verifies in `CMakeCache.txt` that `onnxruntime_USE_AVX/AVX2/AVX512` are all `=OFF` and refuses to continue otherwise. Reasons:
  - **Older CPU intent**: MLAS AVX2/AVX-512 kernel sources are stripped from the build (`onnxruntime_mlas.cmake` lines filtered in the ps1); CI asserts the produced DLL has zero AVX2/AVX512 symbols, and real-device verification on an AVX/SSE4.2 CPU (no AVX2) is PENDING — see the evidence table.
  - **No DLL shipping**: no `onnxruntime.dll` beside the host, no version-skew between the runtime and the DLL.
- **`EXCLUDE_FROM_ALL` bootstraps.** ORT's MSVC build never compiles its `EXCLUDE_FROM_ALL` subprojects, which would leave unresolved externals at the Rust link. The workflow builds them explicitly and stages the archives at the paths ort-sys probes:
  - **re2** (`C2b`): `google/re2 @ 2024-07-02` (ORT's `cmake/deps.txt` pin), built in a wrapper project that `add_subdirectory`s the abseil-cpp source ORT already downloaded (`_deps\abseil_cpp-src`, tag `20250814.0` — re2 2024-07-02 requires abseil), same MSVC Release `/MD` configuration; `re2.lib` is copied to `_deps\re2-build\Release\re2.lib`. Idempotent (skipped when the lib exists) and inside the cached tree.
  - **model_package + utf8_range** (`C2c`): `onnxruntime_session` globs `core/session/model_package/*.cc` and links the standalone model_package subproject; protobuf's `utf8_range`/`utf8_validity` are needed by libprotobuf-lite. Both are built explicitly (`cmake --build --target`) and the archives staged under the ORT build tree.
  - **C3 guard**: asserts `onnxruntime_common.lib`, `re2.lib`, `absl_base.lib`, `model_package.lib`, `utf8_range.lib`, `utf8_validity.lib` and the `_deps\protobuf-build` tree all exist before the Rust link.
- **`dll/` crate = `phk-dll`, cdylib `phk_dll.dll`** (also `rlib` for unit tests). Pinned dependencies: `ort = "=2.0.0-rc.15"`, `image 0.25.10`, `sha2 0.11.1`.
- **ort-sys static-link contract**: `ORT_LIB_PATH` must point at the directory containing `Release\` (the `onnxruntime_*.lib` files) and `_deps\` (protobuf/onnx/nsync/cpuinfo/clog build trees); `ORT_LIB_PROFILE=Release`; `ORT_PREFER_DYNAMIC_LINK=0`; `ORT_SKIP_DOWNLOAD=1` makes a misconfigured build **fail at link time** instead of silently downloading the prebuilt DLL.
- **Default unwinding**: the `dll/` crate release profile deliberately keeps Rust's **default `panic=unwind`** (the old Tauri app profile used `panic=abort`). Every export wraps its work in `catch_unwind`, so a Rust panic is contained and returned as `PHK_ERR_PANIC` (5) instead of terminating the host process. No `RUSTFLAGS` override is needed in CI.
- **No `onnxruntime.dll`**: static linking embeds the ORT/re2/abseil/protobuf archives into `phk_dll.dll`; CI runs `dumpbin /dependents` and fails if `onnxruntime.dll` appears.
- **Model resolution inside the DLL**: `phk_set_models_dir` override → env `PHOTOKHAN_MODELS_DIR` → `<DLL dir>\models` → `<host exe>\models`. The `<DLL dir>` candidate is resolved via `GetModuleHandleExW`/`GetModuleFileNameW` (`current_exe()` returns the HOST exe when the DLL is loaded by another process). The first candidate whose directory contains `manifest.json` wins; the model's SHA-256 is then verified against the manifest (`E004` on mismatch).

## 3. DLL contract

Exports (C ABI, cdecl, UTF-8 paths):

| Export | Contract |
|---|---|
| `int phk_set_models_dir(const char* dir)` | 0 ok; sets the models dir (must contain `manifest.json` + the `.onnx`). 1 bad/empty path; 4 internal failure. |
| `int phk_process(const char* input, const char* outputDir)` | 0 = wrote an ID-photo JPG (preset 3×4, white padding, head straightened) into `outputDir`; 1 = bad args; 2 = model missing/verify failed; 3 = input problem (E001/E002/E003); 4 = pipeline/IO; 5 = panic caught. |
| `int phk_last_error(char* buf, usize cap)` | Copies the last error (UTF-8, empty after a successful call) into `buf`, always NUL-terminated; returns the needed size **including the NUL** — if it exceeds `cap`, call again with a larger buffer. |

Return codes and error mapping:

| rc | Meaning | Last error |
|---|---|---|
| 0 | OK | *(empty)* |
| 1 | `PHK_ERR_ARGS` — null/invalid path argument | message |
| 2 | `PHK_ERR_MODEL` — models dir not found, or model verify failed | `E004: ...` |
| 3 | `PHK_ERR_INPUT` — image unreadable/unsupported (`E001`), no face (`E002`), multiple faces (`E003`) | `E001/E002/E003: ...` |
| 4 | `PHK_ERR_PROCESS` — pipeline/IO failure | `E005–E007: ...` |
| 5 | `PHK_ERR_PANIC` — Rust panic caught at the FFI boundary | `phk_process: panic caught inside DLL` |

Model resolution order: `phk_set_models_dir` → env `PHOTOKHAN_MODELS_DIR` → `<DLL dir>\models` → `<host exe>\models` → **model nhúng trong DLL** (single-file). The first candidate containing `manifest.json` wins, then `ModelStore` verifies the model SHA-256 (mismatch → `E004` → rc 2); with no valid dir the **embedded** model is used — its SHA-256 is verified against the embedded manifest before loading, so **a lone `phk_dll.dll` runs standalone**.

Interop contract (host-side):

- Paths are **UTF-8** — marshal them as NUL-terminated `byte[]` (do **NOT** use `CharSet.Ansi`; it uses the Windows code page and breaks Vietnamese filenames). CI smoke exercises an `ảnh chân dung.png` input.
- `phk_last_error`'s capacity is Rust `usize` (64-bit) — pass `UIntPtr`, not `int`; decode the buffer as UTF-8 up to the NUL.
- The DLL is x64 and imports MSVCP140 (VC++ redistributable required on the target — see section 6).

## 4. Self-contained bundle

Layout: `phk_dll.dll` + `models\` (manifest.json, face_detection_yunet_2023mar.onnx, LICENSE) + `licenses\` + `phk-dll-smoke.cs`. `models\` is optional — the DLL has the model embedded, so **a lone `phk_dll.dll` runs standalone**; keep `models\` only when you want to swap the model without rebuilding.

CI stages and uploads `photokhan-dll-bundle` (artifact `phk-dll-windows-x64`):

```
photokhan-dll-bundle/
├── phk_dll.dll                # the inference DLL (static ORT)
├── models/
│   ├── manifest.json          # from the repo root models/ (committed)
│   ├── face_detection_yunet_2023mar.onnx
│   └── LICENSE                # YuNet MIT
├── licenses/
│   ├── LICENSE-ONNX-Runtime
│   ├── ThirdPartyNotices.txt
│   └── LICENSE-utf8_range
├── phk-dll-smoke.cs           # C# binding: PhkDll.Init(dllPath) - LoadLibrary + GetProcAddress (byte[] + UIntPtr)
└── out/                       # CI smoke output (astronaut.jpg, 354×472)
```

Because the DLL resolves models from its own directory (`<DLL dir>\models`), the bundle is fully self-contained: drop it anywhere, call `phk_process` with nothing configured, and the model is found beside the DLL. CI proves exactly this — it **moves the bundle to a fresh temp directory**, calls `phk_process` with **no `PHOTOKHAN_MODELS_DIR` and no `phk_set_models_dir`**, and asserts the output (section 5).

PowerShell usage (the bundle's `phk-dll-smoke.cs` is the binding; it loads the DLL **at runtime** — `Init(dllPath)` uses `LoadLibrary` + `GetProcAddress` into delegates, so **no CI path is baked in** and the example works from any location):

```powershell
Add-Type -Path photokhan-dll-bundle\phk-dll-smoke.cs
[PhkDll]::Init("$PWD\photokhan-dll-bundle\phk_dll.dll")
function ToUtf8Z([string]$v) { [Text.Encoding]::UTF8.GetBytes($v + [char]0) }
$rc = [PhkDll]::phk_process((ToUtf8Z "photo.jpg"), (ToUtf8Z "out"))
if ($rc -ne 0) {
    $b = New-Object byte[] 1024
    [PhkDll]::phk_last_error($b, [UIntPtr]::new(1024)) | Out-Null
    [Text.Encoding]::UTF8.GetString($b).TrimEnd([char]0)
}
```

## 5. CI reproduction

- Triggers: `workflow_dispatch`, or push / pull_request against `main`. `permissions: contents: read`; concurrency keyed on `github.ref` (cancel-in-progress).
- Job env mirrors the local variables: `ORT_LIB_PATH`, `ORT_LIB_PROFILE=Release`, `ORT_PREFER_DYNAMIC_LINK=0`, `ORT_SKIP_DOWNLOAD=1`, `RUST_BACKTRACE=1`.
- **Warm ORT cache**: the whole source clone + build tree at `D:\build\onnxruntime` is cached with key `ort-win-static-1.31.0-<sha256 of build_ort_baseline.ps1>` and restore-key prefix `ort-win-static-1.31.0-`. Editing the build script forces a new key, but the prefix still restores the previous tree, so the rebuild is incremental (clone reuse + msbuild up-to-date) rather than from scratch. Restore and save are **separate steps**: the save runs right after the ORT stage (`if: steps.C1.outputs.cache-hit != 'true'`), so a failure later in the job still leaves the tree reusable. The C2 step also repairs the cached clone (fetch tag `v1.31.0`, verify HEAD, delete stray same-commit tags) that the script's exact-tag guard would otherwise reject.
- **Rust cache**: `Swatinem/rust-cache@v2` covers the `dll` workspace, keyed `phk-dll-ort-<hashFiles(build_ort_baseline.ps1)>` with `env-vars: ORT_LIB_PATH ORT_LIB_PROFILE ORT_PREFER_DYNAMIC_LINK ORT_SKIP_DOWNLOAD`. Rationale: rustc's default for `-l static` is `+bundle` — ort-sys's rlib COPIES the onnxruntime/re2/abseil archives into itself, and cargo tracks only env vars, so an ORT tree change must also invalidate the Rust cache.
- Steps: A environment → B1 pinned Rust 1.92.0 + rustfmt → B2b Rust cache → B3 cmake ≥ 3.28 guard → C1 restore ORT tree → **C2-Baseline** pristine ORT build (`-SkipStrip`) → C2b re2 → C2c model_package/utf8_range → **D1** baseline DLL (`phk-dll-baseline\phk_dll.dll`) → **C2-Strip** incremental ORT rebuild (MLAS AVX2/AVX512 stripped) → C3 guards → C3b save → **D2** stripped DLL (`phk-dll-stripped\phk_dll.dll`, `avx-symbols.txt` diagnostic — symbol presence is reported, not a fail gate) → **E** compat evidence.
- **E (compat evidence)**: compiles a native console host (`phk-host.exe` via csc) that LoadLibrary + P/Invokes `phk_process`; runs 4 arms — {baseline, stripped} × {native, Intel SDE `-ivb -chip_check`} — on a deterministic input (`astronaut.png`, SHA-256 recorded). SDE (pinned 10.13.1, `downloadmirror.intel.com`) version, `-ivb`/`-chip_check` options and LICENSE are verified. WER LocalDumps are configured for the host; crash metadata (Event Log) uploads as text — raw dumps stay on the runner (public repo, no sensitive memory published). Acceptance = stripped DLL passes native inference **and** SDE `-ivb -chip_check`; baseline under `-ivb` is expected to report a chip-check violation (that is the causal evidence for the i5-3570 hypothesis), reported — not a job failure. Baseline-native rc=0 is a harness sanity gate.
- Artifacts: `phk-dll-windows-x64` (always) = both SHA-identified DLLs, the release bundle (`photokhan-dll-bundle/`, stripped DLL + models + licenses), `report.json`, `avx-symbols.txt`, `avx-disasm.txt`, `dll-dumpbin.txt`, the four test logs, SDE version/help/LICENSE, `crash-report.txt`. `windows-build-diagnostics` (always) = same evidence + ORT build logs + staged MLAS sources. A failed compatibility check is recorded in `report.json` and fails the job — never masked as success.

## 6. Verification evidence

| Claim | Status |
|---|---|
| Build on windows-2022 runner (static ORT, no `onnxruntime.dll` in `phk_dll.dll`) | VERIFIED-BY-CI-RUN (prior runs) |
| Native inference, baseline + stripped DLLs (deterministic input) | PENDING — chờ run đầu của E-compat |
| **Intel SDE Ivy Bridge (`-ivb -chip_check`): stripped DLL passes, baseline DLL reports chip-check violation** | PENDING — chờ run đầu của E-compat (evidence: `test-*-ivb.log`, `report.json` — symbol/disassembly diagnostics in `avx-symbols.txt`, `avx-disasm.txt` are NOT treated as proof of executed instructions) |
| Physical i5-3570 (Windows 10) run | **UNVERIFIED** — reported separately; only a real machine run can confirm the SDE-simulated result |
| Windows 10 runtime | UNVERIFIED (needs a real Windows 10 machine) |
| MSVCP140 / VC++ redist on a clean Windows 10 | UNVERIFIED (present on the runner; the DLL imports `MSVCP140.dll`, so target machines need the Microsoft Visual C++ 2015–2022 x64 Redistributable) |

Evidence classification in `report.json`: **OBSERVED** = native + SDE executions on the runner; **CAUSAL HYPOTHESES** = i5-3570 0xC000001D due to MLAS AVX2 kernel execution (baseline-ivb chip-check expected to confirm); **UNVERIFIED** = physical i5-3570 / Windows 10 / real no-AVX2 hardware. Windows 10 compatibility is NOT asserted from runner evidence alone.

## 7. Licenses

- **YuNet** face detection model — MIT, Shiqi Yu; license text ships as `models/LICENSE` in the bundle.
- **ONNX Runtime v1.31.0** — MIT; statically linked, no runtime files distributed. Full MIT license ships as `licenses\LICENSE-ONNX-Runtime`; `licenses\ThirdPartyNotices.txt` covers its bundled third-party components (RE2, Abseil, ...).
- **utf8_range** (protobuf dependency, statically linked) — MIT, © 2019 Yibo Cai / © 2022 Google LLC; ships as `licenses\LICENSE-utf8_range`.

See `THIRD_PARTY_LICENSES.md` for the project-level license list.

## 8. Troubleshooting

- **ORT cache key / restore-keys**: the cache key embeds the hash of `build_ort_baseline.ps1` — edit that file to force a new key. The restore key prefix (`ort-win-static-1.31.0-`) still pulls the previous tree, so the rebuild starts from the cached checkout/build and is incremental rather than from scratch. If the cached clone's HEAD carries stray tags at the v1.31.0 commit (e.g. `plugin-ep-cuda/v0.3.0`), C2 repairs them (fetch tag, verify HEAD, delete stray same-commit tags) so the script's exact-tag reuse guard passes and the cache key stays intact.
- **`EXCLUDE_FROM_ALL` bootstraps**: ORT's MSVC build never compiles `EXCLUDE_FROM_ALL` projects — re2, model_package and utf8_range stay unbuilt and the Rust link fails with unresolved externals. C2b/C2c build them explicitly and stage the archives at the ort-sys probe paths; C3 guards their presence. Symptom: `unresolved external symbol` at `cargo build --release`. If re2 is missing, check `_deps\re2-build\Release\re2.lib` (the wrapper project must `add_subdirectory` the downloaded abseil source with **real inlined paths** — a previous version passed `-D` cache variables and CMake saw the literal `$VAR` strings, failing the configure).
- **`ORT_LIB_PATH` shape**: it must be the directory containing `Release\` **and** `_deps\` — i.e. `D:\build\onnxruntime\build\Windows\Release`, *not* `...\Release\Release` (that is the profile subdir holding the `.lib` files).
- **`ORT_SKIP_DOWNLOAD` semantics**: `1` makes ort-sys fail the build if the static tree is missing/misconfigured, instead of falling back to downloading the prebuilt `onnxruntime.dll`. Leave it set in CI.
- **dumpbin check**: `dumpbin /dependents phk_dll.dll` must **not** list `onnxruntime.dll` (that would mean dynamic linking). `MSVCP140.dll` imports ARE expected — the target machine needs the **Microsoft Visual C++ 2015–2022 x64 Redistributable**. `api-ms-win-*` entries are UCRT/inbox on Windows 10.
- **UTF-8 marshalling trap**: marshal paths as NUL-terminated `byte[]`; `CharSet.Ansi` uses the Windows code page and breaks Vietnamese filenames (the smoke test's `ảnh chân dung.png` would fail). Pass `UIntPtr` for `phk_last_error`'s capacity — Rust `usize` is 64-bit, `int` is 32-bit.
- **panic=unwind note**: the `dll/` crate release profile keeps DEFAULT unwind so `catch_unwind` can contain Rust panics as `PHK_ERR_PANIC` (5). Do not add `panic=abort` to the dll release profile — a panic would then terminate the host process.
