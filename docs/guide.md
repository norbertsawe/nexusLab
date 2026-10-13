# User guide

A native Rust scientific workbench with a script editor, command window,
editable data, persistent workspaces, figures, live acquisition, and a headless
automation API. Windows 10/11 x64 is
the primary target. The same source supports Linux and macOS.

![NexusLab Windows-style workbench](images/workbench-dark.png)

## Start

Install stable Rust and, on Windows, Visual Studio Build Tools with the **Desktop
development with C++** workload and Windows SDK. No runtime installer is needed
for the release application. From this directory:

```powershell
cargo run
# Equivalent:
cargo run --package nexus-gui
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --release --package nexus-gui --target x86_64-pc-windows-msvc --locked
```

The desktop opens the **Workbench**, with an analysis script ready to run.
Press **F5** to fit measured data, calculate residuals and RMSE, and create a
scatter plot with a fitted line. Open **Figures** to inspect the result or click
a workspace variable to edit its values. **Live Scope** opens the hardware
manager and starts the 128 kSa/s mock source for hardware-free testing.

## Desktop workflow

The interface uses Segoe UI Variable and Cascadia Mono when installed (with
Segoe UI/Consolas fallbacks), Windows control colors, a standard menu/toolbar,
compact rectangular controls, and white editor and plot surfaces. The explorer
has full-width alternating rows, clear selection/hover states, and vector icons
for folders, scripts, datasets, workspaces, settings, and documents. Templates
are available under **Tools → New from template**; commands and shortcuts
are under **Help**.

**Edit → Preferences** (`Ctrl+,`) selects Light, Dark, or System appearance and
adjusts interface/code font sizes, striped explorer/table rows, editor line
numbers, the action toolbar, status bar, and startup layout restoration.
Changes apply immediately and save to `preferences.json` beside draft recovery.

- **Script editor:** multiple tabs, MATLAB/Nexus and Rhai modes, syntax colors,
  line numbers, cursor position, selection execution, find/replace, native
  Open/Save/Save As dialogs, and reusable analysis/matrix/filtering templates.
- **Command window:** expression results, matrix previews, printed output,
  errors and elapsed time, searchable/replayable history, and transcript export.
  **Tab** opens variable/function completion (Up/Down, Enter, Escape).
  **F6** expands it. Evaluation runs on a worker; **Stop** cancels script work
  cooperatively.
- **Workspace and variable editor:** filtered variables with types, dimensions,
  and sizes; editable numeric cells, column paging, deletion, and plotting
  selected columns. Apply edits explicitly or let the next operation commit
  them. Complex arrays and text are inspectable but read-only.
- **Files and persistence:** current-folder browser, numeric CSV import/export,
  and validated `.nxw` workspace Save/Load. Opening a CSV imports it as a named
  matrix; opening a `.nxw` replaces the numerical workspace.
- **Figures:** separate numbered figures with line/scatter/bar series, overlays,
  titles, axis labels, grid, legend, crosshair/nearest data coordinates,
  autoscale, SVG export, PNG at 4000 × 2400 / 300 DPI, and raw series CSV export.
  Wheel zooms both axes; Ctrl+wheel zooms X, Shift+wheel zooms Y. Middle or left
  drag pans the canvas. CSV exports every original point; drawn/exported images
  use a bounded sample of each series.
- **DSP toolkit:** **Tools → Filter Design**, **Spectral Analyzer**, and
  **Statistics Summary**. Filter Design creates windowed-sinc FIR taps or
  prewarped Butterworth IIR second-order sections, with a response plot. PSD
  uses one-sided Welch density (Hann window, 50% overlap, segment detrending).
  Statistics include mean, median, sample standard deviation, RMS and range;
  complex arrays use magnitudes.
- **About and diagnostics:** **Help → About NexusLab** credits author and lead
  architect **Norbert John**. Its diagnostics tab shows the WGPU adapter/backend,
  resident process memory, workspace payload, active OS threads and worker state.
- **Live scope:** serial or mock acquisition, pause, rising-edge trigger,
  waveform settings, and a cached 4096-point Hann-windowed FFT at 10 Hz.

Click the **pop-out icon** in any panel or use **Window → Detach …** to open a genuine
native window. Editor, command window, figures, variable editor, live scope,
workspace, folder browser, and hardware manager all detach. They share the
same execution session and data. Click the **snap-in icon** or choose
**Window → Dock all panels** to return panels to the main window. The **× icon**
on each panel, and the native close button on a detached window, close that
panel while retaining its documents and data. Reopen panels from **View** or
**Window**.

**View → Close all panels** (`Ctrl+Shift+W`) leaves a minimal start screen with
a NexusLab waveform mark and shortcuts. **Restore panels** (`Ctrl+Shift+P`)
returns to the workbench; `F6` reopens the command window. Closing the editor
panel keeps unsaved documents. Closing an individual document tab retains its
normal save/discard prompt. Open, detached, and closed panel choices are restored
with drafts on the next launch when layout restoration is enabled.

Editor drafts, command history, and the current folder recover automatically
from `%LOCALAPPDATA%\NexusLab\recovery.json` on Windows (XDG config on Unix).
Unsaved edits prompt before closing. Numerical variables require **Save
workspace**; draft recovery does not save them. Normal script saves detect
external file changes and preserve your edits so you can review or Save As.

| Shortcut | Action |
| --- | --- |
| Ctrl+N / Ctrl+O | New script / Open file |
| Ctrl+S / Ctrl+Shift+S | Save / Save As |
| F5 / Ctrl+Enter | Run current script |
| Ctrl+Shift+Enter | Run selected code |
| Ctrl+F | Find and replace |
| F6 | Expand/focus command window |
| Ctrl+L | Clear command output |
| Ctrl+, | Preferences |
| Ctrl+Shift+W | Close all panels |
| Ctrl+Shift+P | Restore panels |

The release application is `target/x86_64-pc-windows-msvc/release/nexuslab.exe`.
The locally built portable executable is also available as `dist/nexuslab.exe`.
Copy that executable alone. Fallback fonts, math, interpreter, GUI, and API are compiled
in. Static CRT linking removes the MSVC redistributable requirement. Windows
system libraries and an installed GPU driver are still required. WGPU selects
DX12/Vulkan on Windows and Metal on macOS; this is not a software renderer.

Linux build prerequisites: a C compiler, pkg-config, X11/Wayland development
libraries and a Vulkan driver. For Ubuntu: `build-essential pkg-config
libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxkbcommon-dev
libwayland-dev libudev-dev`. macOS requires Xcode command line tools and Metal.

## Console and scripts

Enter expressions in the command window and press Enter or Evaluate. Up/down
arrows recall history. The desktop starts with `pi`; headless sessions also
provide example `t` and `sig1` arrays. Newlines display results; semicolons
suppress ordinary statement output. `disp` prints explicitly.

```matlab
t = 0:0.00001:0.01;
sig = sin(2 * pi * 1000 * t) + 0.05 * randn(size(t));
fft_result = fft(sig);
plot(t, sig)
A = [1, 2; 3, 4]; B = A * A; inverse = inv(A)
```

Supported syntax: assignment, multiple statements, `%` comments, numeric/scientific
notation, parentheses, row/column matrices, colon ranges, transpose `'`, scalar
broadcast, matrix `*`, elementwise `.*`, `./`, `.^`, and linear solve `A \ b`.
Functions include `sin`, `cos`, `tan`, `sqrt`, `exp`, `log`, `abs`, `fft`, `ifft`,
`transpose`, `inv`, `solve`, `size`, `length`, `sum`, `mean`, `linspace`,
`zeros`, `ones`, `randn`, `hann`, `hamming`, `blackmanharris`, `fir`, `plot`,
`fir_lowpass`, `fir_highpass`, `fir_bandpass`, `iir_lowpass`, `iir_highpass`,
`iir_bandpass`, `serial_ports`, `usb_devices`. `randn(size(t))` preserves shape. `fft` produces
complex samples; `abs` and `ifft` consume them. Also available: `det`, `eye`,
`diag`, `reshape`, `polyfit`, `polyval`, `min`, `max`, `std`, `norm`, `diff`,
`cumsum`, `sort`, `log10`, `floor`, `ceil`, `round`, `row`, and `column`.
`help`, `who`, `whos`, `clear`, and `clear name` manage the session; desktop
commands `clc`, `pwd`, `cd folder`, and `run file` manage the workbench.
Figures accept `figure(n)`, `plot(y)`, `plot(x,y)`, `scatter(x,y)`, `bar(y)`,
`hold('on')`, `grid('off')`, `title('...')`, `xlabel('...')`, and `ylabel('...')`.
`close all` clears figures. `A(r,c)` and `A(n)` provide scalar 1-based indexing;
linear indexing and `reshape` use column-major order. Version 0.4 also supports
`A(:,2)`, `A(1:end)`, vector subscripts, indexed assignment, `eig`, `pinv`,
integer matrix powers, right matrix division, and rectangular constructors.
Open **Tutorial** for NexusAcademy and runnable examples. See the
[compatibility reference](compatibility.md) for exact semantics and migration
notes.

v0.3 DSP functions: `stats(x)`, `median(x)`, `rms(x)`, `ptp(x)`,
`psd(x, sample_rate, segment_length)`, `butter_lowpass(order, cutoff, fs)`,
`butter_highpass(order, cutoff, fs)`, `butter_bandpass(order, low, high, fs)`,
`sosfilt(x, sos)`, `filter(x, taps)` (FIR alias), and `freqz(coefficients, fs)`.
PSD returns frequency/power-density columns; freqz returns frequency/gain-dB
columns. Butterworth orders are 1–12; band-pass digital order is twice the
prototype order. FIR tap counts are odd. These functions also run through
the CLI and the automation API's `execute` tool.

Use `rhai:` to evaluate sandboxed Rhai with loops and conditionals. Numeric
variables are shared with the inspector. `.rhai` files select this mode
automatically. Native Rhai bindings include `sin`, `cos`, `sqrt`, `fft`, and
`serial_ports`. Scripts have no built-in filesystem or network access.
Printed output is captured in evaluation results, including over MCP; complex
FFT arrays are available in the shared workspace.

```powershell
.\target\debug\nexuslab.exe run examples/simulation.nx
.\target\debug\nexuslab.exe run examples/matrices.m --json
.\target\debug\nexuslab.exe run examples/loops.rhai
.\target\debug\nexuslab.exe eval 'x = 0:0.1:1; sum(x)' --json
# A smaller build without the graphics frontend:
cargo run --package nexus-cli -- run examples/simulation.nx
```

This is a deliberately limited M-file subset, not MATLAB compatibility: no
toolboxes, user-defined M-file functions, slices/indexed assignment, symbolic
math, or MATLAB control flow. `^` and
`/` currently use elementwise numeric semantics; use `*` for matrix products
and `solve`/`\` for systems. Reduction functions flatten arrays. Rhai does
not preserve matrix dimensions. See DESIGN.md for resource and performance limits.

## Hardware

The manager enumerates actual devices and also offers COM1–COM256 on Windows.
Enter a Unix device path on other platforms. Set a custom baud rate, parity,
flow control, and framing, then Connect. Disconnect/reconnect runs outside the
UI with a one-second retry interval and bounded read timeouts.

- **Text lines:** one finite numeric amplitude per LF-terminated line, with CRLF accepted.
- **Float32 LE:** consecutive finite little-endian f32 amplitudes; partial reads are retained.

The 128 kHz mock is independent of serial wire speed. At 115200 baud, an 8N1
connection carries at most about 11.5 kB/s; binary f32 telemetry is therefore
limited to about 2.9 kSa/s. Configure the device rate before interpreting
time or frequency axes for physical telemetry, using the **Sa/s** field.

`nexus-hardware` also exposes USB enumeration and asynchronous direct bulk IN
and interrupt report transfers using nusb, SCPI compound command parsing/generation and bounded TCP
queries. USB access on Windows requires an appropriate WinUSB-compatible
device/interface driver. USB and SCPI are library interfaces in this version;
the desktop manager connects serial telemetry.

## Automation API

```powershell
.\target\debug\nexuslab.exe serve --port 8080
.\target\debug\nexuslab.exe serve --stdio
```

HTTP binds only `127.0.0.1`. POST JSON-RPC to `/rpc` or `/mcp`; `/health` reports
readiness. MCP supports `initialize`, `ping`, `tools/list`, `tools/call`,
`resources/list`, `resources/read`, and notifications. HTTP uses stateless JSON
responses; stdio uses newline-delimited JSON, with logs on stderr. The workspace
is persistent within each server process. It starts with mock acquisition.

```powershell
$request = @{jsonrpc='2.0'; id=1; method='execute'; params=@{code='x = sin(0:0.1:1)'}} | ConvertTo-Json -Depth 8
Invoke-RestMethod http://127.0.0.1:8080/rpc -Method Post -ContentType application/json -Body $request
```

Tools: `execute(code)`, `inspect(name?)`, `list_ports()`, `read_samples(limit?)`,
`connect(port, baud?, sample_rate?)`, `disconnect()`. Workspace resource: `nexus://workspace`.
For an MCP client, configure the release executable as the command and
`["serve", "--stdio"]` as its arguments. Local clients may execute scripts and
control acquisition; do not expose the unauthenticated endpoint to a network.

## Project

| Crate | Responsibility |
| --- | --- |
| nexus-core | Arrays, linear algebra, FFT, windows, FIR/IIR, signal synthesis |
| nexus-script | AST parser, bounded Rhai, figures, validated workspace/CSV I/O |
| nexus-hardware | Lock-free acquisition queue, serial, direct USB, SCPI |
| nexus-gui | Editor, commands, data tables, figures, files, native detachable panels, scope |
| nexus-cli | File/eval runner, JSON-RPC, MCP HTTP and stdio |

Architecture, tradeoffs, and validation details are in [architecture notes](../DESIGN.md).
Dependencies are pinned by Cargo.lock. Licensed under MIT.

Windows validation results and measured limits are recorded in
[testing notes](testing.md). To run the local HTTP smoke
check: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke.ps1`.
Interactive desktop verification:
`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/gui-smoke.ps1`.
To measure hardware-free acquisition:
`cargo run --release --package nexus-hardware --example throughput`.
