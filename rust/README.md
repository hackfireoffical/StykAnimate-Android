# StykAnimate, Rust prototype

A native (no WebView) Android app built with Rust, `eframe`/`egui` and `cargo-apk`.

- **Phase 1:** builds an APK in GitHub Actions and shows a native timeline with an animated
  stick figure. Purpose: prove the toolchain and how it feels on a phone.
- **Phase 2a (`styk-format/`):** a pure-Rust reader for `.styk` / `.stkz` / `.stykmov`
  containers (v1 and v2), ported from `backend/native_format.py`. Tested by the "Rust format
  tests" workflow.
- **Next:** an Android file picker (needs JNI), then reading the scene JSON (figures, poses,
  timeline) and drawing real frames. `.piv` / `.stk` import comes after native files work.

Build the APK: run the "Rust Android APK" workflow on the `rust-prototype` branch.
