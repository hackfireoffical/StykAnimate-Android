<h1 align="center">StykDroid</h1>

<p align="center">
  An unofficial Android port of <a href="https://stykanimate.com">StykAnimate</a>, the free, open-source stick figure animation app.<br>
  Opens Pivot (<code>.piv</code>, <code>.stk</code>) and Stick Nodes projects, and keeps your work on your device.
</p>

<p align="center">
  <a href="https://github.com/hackfireoffical/StykAnimate-Android/actions">Builds</a> ·
  <a href="https://stykanimate.com">Upstream website</a> ·
  <a href="https://github.com/filippocalippo/StykAnimate-App">Upstream repo</a>
</p>

> **Status:** work in progress. This is a personal fork, built and tested on a phone. Expect rough edges.

## What this is

StykDroid wraps the StykAnimate editor in a Capacitor Android app and tunes it to feel like a real app instead of a web page in a frame: edge-to-edge layout that stays out of the status and navigation bars, a proper Android back button (closes dialogs, steps back through the app, double-press to exit), and a phone-friendly layout for the export footer and other screens.

In parallel, there is an experimental native Rust version built on egui. It is not usable yet.

## Branches

| Branch | What it is |
| --- | --- |
| `main` | Upstream StykAnimate code plus this README |
| `android-capacitor` | The Capacitor Android wrapper and APK build workflow |
| `android-app-feel` | Native-feel work on the WebView app: back button, insets, mobile CSS |
| `rust-prototype` | Native Rust/egui prototype (`styk-format`, `styk-scene`, `styk-geometry` crates) |

## Get the APK

There are no store releases yet. Builds come from GitHub Actions:

1. Open the [Actions tab](https://github.com/hackfireoffical/StykAnimate-Android/actions).
2. Pick the latest successful **Android APK** run.
3. Download the APK from the run's artifacts (GitHub requires you to be signed in).
4. Install it on your phone. You may need to allow installs from your browser or file manager.

## Features (from StykAnimate)

- **Pose stick figures** with fixed-length joints, IK for arms and legs, stretchy nodes and 3D figures with depth.
- **Build your own figures** from lines, circles, polygons, text and images, with a shared rig and per-part styles.
- **Two timelines:** a Simple, Pivot-style frame strip and an Advanced timeline with layers, folders, keys and easing.
- **Motion paths, onion skins and camera moves,** including camera follow.
- **Particle effects** (explosions, auras, beams, smoke) and brushes for drawing.
- **Export** MP4, WebM (with transparency), GIF, APNG, PNG/JPEG/SVG frames, and editable `.piv`/`.stk`.
- **27 interface languages**, light and dark themes, and a touch interface for phones and tablets.

Everything (decoding, editing, saving and rendering) runs locally. Upstream's optional, consent-based diagnostics are described in [docs/telemetry.md](docs/telemetry.md).

## Build from source

Android builds run in CI (`.github/workflows/android.yml`), so you don't need a PC. The workflow installs the Capacitor plugins, syncs, and builds the APK.

To run the editor itself:

```sh
npm ci
python -m pip install -r backend/requirements-fast.txt
npm run serve        # the editor in your browser at http://127.0.0.1:47831
```

Requirements: Node.js 22+ and Python 3.11+. See [Web build](docs/WEB.md) and [Touch interface](docs/TOUCH.md). The desktop (Electron) build still exists from upstream: `npm start`.

## Tests

```sh
npm run check        # syntax, stylesheet lint
npm test             # unit tests
python -m unittest discover -s backend -p "test_*.py"
```

For the Rust prototype, the `Rust format tests` workflow runs the crate tests on `rust-prototype`.

## Credits

StykDroid is a fork of [filippocalippo/StykAnimate-App](https://github.com/filippocalippo/StykAnimate-App). All the animation editor work is theirs. This fork only adds the Android packaging and mobile polish.

## Contributing

Bug reports and pull requests are welcome. Upstream's [CONTRIBUTING.md](CONTRIBUTING.md) and [Code of Conduct](CODE_OF_CONDUCT.md) apply. Report security problems privately as described in [SECURITY.md](SECURITY.md).

## License

StykAnimate is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE), version 3 or (at your option) any later version. Bundled third-party components keep their own licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Trademarks

StykDroid is an unofficial fork and is not the official StykAnimate release. The StykAnimate name and logo identify the official project, so this fork uses its own name and does not use the official logo.

StykAnimate is an independent project. It is not affiliated with or endorsed by Pivot Animator or Stick Nodes; those names are used only to describe file compatibility.
