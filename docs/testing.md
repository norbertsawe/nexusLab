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

The initial v0.3.0 Windows validation passed all **28 tests**, formatting,
`cargo check`, and Clippy with warnings treated as errors. Cross-platform CI
runs formatting, Clippy, tests, and builds on Windows, Linux, and macOS.

## Desktop verification

Build the GUI first:

```powershell
cargo build --locked --package nexus-gui
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-v03-extras.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-preferences-smoke.ps1
```

The desktop scripts require an interactive Windows session and use the mouse
and keyboard. Run them one at a time. Captures, saved workspaces, and logs are
written to the ignored `artifacts/` directory.

These checks exercise script execution, command completion and history, file
dialogs, data import, figure exports, detached windows, filter design, spectral
analysis, statistics, theme changes, and layout recovery. The preferences test
uses an isolated profile and checks that closing panels retains session data.

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
