# Architecture

## Dependency direction

`core <- hardware <- script <- cli <- gui`, with script and gui also depending
directly on core. Only gui depends on egui/WGPU. The standalone headless CLI
does not initialize a window, GPU, or event loop. The nexuslab desktop binary
dispatches CLI arguments before graphics initialization.

## Data path and threading

One acquisition thread owns the serial handle and generator. A bounded
single-producer/single-consumer ringbuf queue stores 262144 timestamp/amplitude
pairs (4 MiB). The producer never waits for the UI; full queues drop new samples
and increment a visible counter. No allocation or mutex is needed per mock
sample. A callback API borrows ring storage directly for zero-copy consumers.
The GUI drains bounded batches and retains 32768 recent samples,
rendering 1280 scope points. The 4096-point FFT plan, scratch, and Hann window
are cached, and spectra refresh at 10 Hz. Plot rendering and the rolling history
copy sample values; the application does not claim end-to-end zero-copy.

Serial uses serialport's native Win32 implementation on Windows; OS enumeration
is supplemented with a COM1–COM256 selector, which does not imply every port
exists. Twenty-millisecond reads bound shutdown latency. Device loss retries
once per second. A framing decoder carries partial messages across reads,
rejects nonfinite amplitudes, and caps malformed line carry at 64 KiB. USB
transfers use nusb for asynchronous bulk and interrupt I/O at the library
boundary. Device-specific descriptors, permissions, drivers, and SCPI
command meanings remain the instrument's responsibility.

The GUI uses another worker for evaluation and data/file operations. It owns a
single Session shared by commands, editor, workspace and figures. Requests and
result snapshots pass through channels. The GUI accepts one outstanding
operation. Pending numeric table edits travel with the next request and are
validated before committing; workspace loads validate before replacement.
Stop sets an atomic flag checked between AST statements and by Rhai's progress
callback. An in-progress math operation finishes before noticing cancellation.
The API uses spawn_blocking and a mutex to serialize workspace mutations.
Thus script execution never owns or blocks the acquisition loop.

## Mathematics and language

Core Matrix stores row-major f64 values; nalgebra supplies matrix products,
inverse, and LU solve. Tensor wraps ndarray ArrayD for checked N-D storage and
axis permutation. rustfft handles FFT/IFFT. FIR uses windowed sinc, spectral
inversion, and low-pass subtraction; IIR uses normalized RBJ biquads with state.
The numerical backends use native Rust libraries without a separate numerical
runtime.

The M-file interpreter lexes into tokens, constructs a Pratt-parsed AST, and
evaluates against a BTreeMap workspace. Assignments commit statement by
statement, so a later error preserves successful earlier statements. Colon
ranges validate finite inputs and count before allocation. Matrix `*` and
scalar broadcasting are distinct; `.*` is elementwise. Scalar indexing and
reshape use MATLAB-style column-major order over the row-major storage.
Newlines preserve displayed output while semicolons suppress it. Strings use
single or double quotes; postfix apostrophes transpose matrices. Polynomial
fitting uses a scaled, bounded SVD least-squares solve with a maximum degree
of 16. Schur decomposition provides real/complex eigenvalues; scaled SVD
provides pseudoinverses and rectangular least-squares solutions. Decompositions
reject nonfinite inputs and have dimension and iteration limits.

Slicing and indexed assignment share a checked selection implementation.
`end` uses a restored subscript context, and `:` selects an entire dimension.
Selections preserve MATLAB-style column-major order. Indexed assignment stages
the full updated array before committing, so invalid dimensions or indices
cannot partially replace values. Complex column vectors extend the serialized
value model for eigenvalues without changing existing complex FFT rows.

NexusAcademy is a detachable, closable panel with a resizable contents tree,
lesson/reference views, and themed code blocks using the editor highlighter.
Try it submits the example through the existing command execution path, sharing
history, cancellation, worker execution, and workspace state. Every example is
tested independently and in sequence in a shared Session.

The canonical app mark is `assets/nexuslab.svg`. A Rust example regenerates
PNG and seven-size ICO assets; Windows build resources embed the ICO and
version metadata. Main and detached viewports share the cached native icon.

Figures are backend data models, independent of egui. Sessions support at most
16 numbered figures, 32 series per figure and four million total points.
Rendering and SVG export decimate to about 8192 points per series and preserve
the final sample.
These figures are separate from the live acquisition scope.

Panel headers use vector icons; the status bar separates connection,
execution, and workspace information. About/diagnostics credits Norbert John and
reports measured resident memory and OS thread count (Win32 or Linux procfs).
Graphics adapter details stay in diagnostics. Completion uses a bounded,
keyboard-navigable popup anchored to the REPL and preserves the caret when
inserting a function or workspace identifier.

DSP tools submit work to the existing Session worker. Butterworth design uses
analog prototype poles, frequency prewarping and a bilinear transform, with
conjugate poles paired into normalized real second-order sections. Band-pass
transformation doubles the prototype order. FIR remains windowed-sinc and is
labeled separately. Welch PSD caches its FFT plan/scratch buffer and uses a
Hann window, 50% segment overlap, segment mean removal and one-sided density
normalization. Statistics use N−1 for sample standard deviation and complex
magnitudes for complex arrays. All operations reject invalid parameters and
nonfinite results.

The plot canvas overlays crosshairs without changing data bounds. Wheel zoom
can target X or Y independently; middle/left drag translates bounds. Image
exports rasterize SVG on the worker using resvg, at 4000 × 2400 with 300-DPI
PNG metadata. Raw figure CSV preserves all source points and quotes series
names. SVG/PNG use the bounded visual sample. Export writes remain atomic.

Workspace files carry a format identifier and version. Names, matrix shapes,
finite values, variable counts and memory limits are checked before loading.
Numeric CSV import accepts a rectangular table with an optional header row.
Writes use a temporary file in the target directory followed by atomic
replacement. CSV export preserves rectangular real matrices and represents
complex values as real/imaginary columns.

Editor documents track saved text for dirty state and external-change
detection. Recovery stores drafts, history, current folder and detached panel
keys; it does not snapshot numerical variables. Recovery writes run separately
from painting every 30 seconds and at exit. The GUI's files, editor, figures,
scope, worker and docking are separate modules.

Limits: 64 KiB source, one million elements per generated array, 128 parser
nesting levels, 64 MiB M-file workspace, 50 million estimated multiplication/FIR
operations, inverse/solve dimensions up to 256. Rhai limits operations to
200000, call depth to 32, expression depth to 64/32, array length to one million,
and string length to 65536. These are cooperative resource limits, not process
isolation or a guarantee against all hostile input. Rhai variables retain their
scope between evaluations and share scalar/flat real-array values with the
workspace. M-file arrays are f64; FFT output is complex f64 pairs.

## Graphics and portability

eframe 0.31.1 and egui_plot 0.32.1 are intentionally paired. The desktop uses a
compact Windows-style interface with light/dark/system appearance: a conventional menu bar and toolbar,
rectangular controls, gray pane surfaces, white editing/plot areas and table
rows. The explorer uses full-width alternating rows, file-type vector icons,
ellipsized names, and distinct selection, hover and keyboard-focus states.
The GUI reads Windows system control/text/selection colors and loads Segoe UI
Variable/Cascadia Mono from the installed Windows Fonts directory, falling back
to Segoe UI/Consolas when those fonts are unavailable. Embedded
egui fonts remain fallback fonts; no font files ship with the application.
The selected theme supplies every panel, dialog, editor syntax palette, plot
series palette and explorer icon/row palette. Main and detached windows request
matching OS title bars. System mode follows reported OS theme changes. egui
renders controls, and winit provides native application windows. Connection
information appears in the status bar; graphics information is available in
diagnostics. SVG/PNG exports use a white background for sharing and printing.

Preferences are bounded, defaulted JSON stored atomically beside recovery.
Changing a setting reapplies styling and updates existing detached windows;
font files are installed once at startup. Closing a panel hides its UI without
deleting documents or numerical data. Recovery records closed panel keys and
the active central view with backwards-compatible defaults. Native child close
requests now close panels; only an explicit snap-in action docks them. Closing
all panels leaves a vector start screen, and View/Window menus remain available
to reopen the UI. Reopening panels does not create a new execution Session.

WGPU explicitly compiles DX12, Vulkan, and Metal. Windows adapter selection
prefers a compatible DX12 GPU, then Vulkan; CPU adapters have lower priority.
Panels use stable egui viewport IDs and immediate native viewports. Closing a
child viewport closes its panel without deleting shared data. The snap-in
control returns the panel to the main window.
All panels draw from the same application state and one script worker. A
backend without native viewports falls back to an embedded egui window.

Vsync and continuous repaint while viewing the live scope target the monitor's
refresh rate; workbench and paused views repaint at 10 Hz.
Measured throughput depends on GPU, monitor, driver,
power state, and build profile. This is a target, not a universal 60+ FPS
guarantee. Sampling and painting rates are independent.

The MSVC target uses `+crt-static`. OS DLLs and graphics/USB drivers are still
required. The portable executable runs without separately distributed
application DLLs or an MSVC redistributable. WGPU loads installed system
graphics components at runtime.
Linux/macOS builds are source-compatible targets; local Windows verification
does not establish that they were tested on those platforms.

## API protocol

One binary supports GUI, run, eval, devices, and serve. The API provides
JSON-RPC 2.0 with persistent workspace state and MCP tool/resource discovery.
MCP advertises protocol 2025-03-26. The HTTP endpoint returns JSON results
without an SSE stream or session ID; stdio provides the broadest client
compatibility. Request body limit is 128 KiB. Notifications produce no JSON-RPC
response. MCP execution failures are tool results with isError; ordinary RPC
failures use error objects. The service is local and unauthenticated.

## Current boundaries

This v0.3 workbench implements a useful numerical subset. M-file user-defined
functions/control flow, slicing/indexed assignment, graphical dataflow programming,
device-specific USB decoding, HID report descriptors, SCPI indefinite #0 blocks,
and multi-instrument synchronization need further work. Workspace files save
variables; figure layouts and hardware connections are not included in them.
SCPI handles general text headers, queries, arguments, quoted semicolons, and
compound commands, typed arguments, units, radix numbers, and IEEE 488.2
definite-length binary blocks; it does not implement instrument-specific command trees.
Physical telemetry defaults to 128 kSa/s for timestamps and FFT axes; set the
device's true sampling rate in the hardware panel or API before measurement
use. The mock provides a known clock for hardware-free validation.

## References

- [Butterworth filter design and SOS guidance](https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.butter.html)
- [eframe NativeOptions](https://docs.rs/eframe/0.31.1/eframe/struct.NativeOptions.html)
- [egui WGPU setup](https://docs.rs/egui-wgpu/0.31.1/egui_wgpu/struct.WgpuSetupCreateNew.html)
- [nusb interface transfers](https://docs.rs/nusb/0.1.14/nusb/struct.Interface.html)
- [MCP transport specification](https://modelcontextprotocol.io/specification/2025-03-26/basic/transports)
