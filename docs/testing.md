# Testing

## Development checks

Run these checks from the repository root:

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
```

The test suite covers numerical operations, Butterworth filter stability and
cutoff response, PSD normalization, script evaluation, workspace validation,
CSV handling, document recovery, figure export, and protocol behavior.
The v0.4 additions check pseudoinverse identities for rectangular and
rank-deficient arrays, real/complex eigenvalues, matrix powers, slicing and
atomic indexed assignment, complex-column persistence, and every Academy
example in both a fresh session and a shared workspace.

The initial v0.3.0 Windows validation passed all **28 tests**, formatting,
`cargo check`, and Clippy with warnings treated as errors. Cross-platform CI
runs formatting, Clippy, tests, and builds on Windows, Linux, and macOS.

The v0.4.0 Windows validation passes **36 tests**, formatting, `cargo check`,
Clippy with warnings treated as errors, and the optimized release build. The
Academy desktop test passes for the debug and packaged executables; the
preferences desktop test and CLI/API smoke tests also pass. Windows executable metadata and icon extraction
are also verified. Cross-platform builds of this update have not been run
locally.

## Desktop verification

Build the GUI first:

```powershell
cargo build --locked --package nexus-gui
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-v03-extras.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-preferences-smoke.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-academy-smoke.ps1
```

The desktop scripts require an interactive Windows session and use the mouse
and keyboard. Run them one at a time. Captures, saved workspaces, and logs are
written to the ignored `artifacts/` directory.

These checks exercise script execution, command completion and history, file
dialogs, data import, figure exports, detached windows, filter design, spectral
analysis, statistics, theme changes, and layout recovery. The preferences test
uses an isolated profile and checks that closing panels retains session data.
The Academy test verifies Try it execution, eigenvalues and pseudoinverses in
the shared workspace, dark mode, native Tutorial detachment, command history,
and closing all nine panels. It temporarily brings its own test windows to the
front and stops if they fail to receive focus.

To regenerate application icon assets after editing the SVG:

```powershell
cargo run --locked --package nexus-gui --example generate_icons
```

Windows builds embed the ICO and executable version metadata through the
Windows SDK resource compiler. A unit test checks every ICO frame and the
native window's RGBA icon data.

To verify a release build, pass its executable using `-Binary`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-preferences-smoke.ps1 -Binary target/x86_64-pc-windows-msvc/release/nexuslab.exe
```

## CLI and API checks

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke.ps1
cargo run --release --package nexus-hardware --example throughput
```

The smoke script checks script files, numerical evaluation, JSON-RPC/MCP,
workspace state, simulated acquisition, notifications, request errors, and
Origin validation. The throughput example measures the built-in source.

Simulated acquisition verifies the software data path. Physical serial, USB,
and SCPI devices require separate checks with the intended hardware.
