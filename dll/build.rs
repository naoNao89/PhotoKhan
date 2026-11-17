fn main() {
    // ORT 1.31's Windows telemetry (1DS SDK, onnxruntime_USE_TELEMETRY default ON)
    // compiles NetworkDetector.obj which references USER32 message APIs
    // (TranslateMessage/DispatchMessageW/PeekMessageW/PostThreadMessageW/
    // MsgWaitForMultipleObjectsEx). ort-sys emits shell32 but not user32; the
    // Tauri app crate gets user32 transitively, this standalone DLL must link it.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=user32");
    }
}
