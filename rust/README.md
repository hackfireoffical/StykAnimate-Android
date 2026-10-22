# StykAnimate, Rust prototype

A native (no WebView) Android app built with Rust, `eframe`/`egui` and `cargo-apk`.

- Phase 1 (this commit): builds an APK in GitHub Actions and shows a native timeline with an
  animated stick figure. Purpose: prove the toolchain and how it feels on a phone.
- Phase 2: open a real `.piv` / `.stk` file and play its frames (format notes live in
  `backend/native_format.py` and `src/pivot/`).

Build: run the "Rust Android APK" workflow on the `rust-prototype` branch.
