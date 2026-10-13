use crate::{
    documents::{self, Documents, Recovery},
    theme::{self, BLUE, GREEN},
};
use eframe::egui::{self, RichText};
use egui_plot::{HLine, Legend, Line, Plot, PlotPoints};
use nexus_core::{Complex, Matrix, Waveform, Window};
use nexus_hardware::{Acquisition, Framing, PortInfo, SerialConfig};
use nexus_script::{Figure, Session, Value};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{atomic::Ordering, mpsc, Arc},
    thread::JoinHandle,
    time::{Duration, Instant},
};

mod about;
mod academy;
mod completion;
mod docking;
mod editor;
mod figures;
mod files;
mod preferences_ui;
mod scope;
mod toolkit;
mod ui;
mod welcome;
mod worker;
use docking::Panel;
use worker::{Job, Request, ScriptWorker};

#[derive(Clone, Copy, Debug, PartialEq)]
enum View {
    Welcome,
    Workbench,
    Figures,
    Variables,
    Scope,
    Command,
    Help,
    Tutorial,
}
enum Confirm {
    CloseTab(usize),
    Exit,
}
#[derive(Clone)]
struct FileEntry {
    path: PathBuf,
    name: String,
    directory: bool,
}

pub struct NexusApp {
    preferences: crate::preferences::Preferences,
    preferences_open: bool,
    applied_dark: bool,
    closed_panels: BTreeSet<Panel>,
    hardware_visible: bool,
    toolkit: toolkit::Toolkit,
    about_open: bool,
    about_diagnostics: bool,
    diagnostics: crate::diagnostics::Diagnostics,
    diagnostics_at: Instant,
    graphics: String,
    completion: completion::Completion,
    detached: BTreeSet<Panel>,
    themed_viewports: BTreeSet<Panel>,
    focus_panel: Option<Panel>,
    capture: Option<PathBuf>,
    capture_requested: bool,
    capture_at: Instant,
    acquisition: Acquisition,
    samples: VecDeque<[f64; 2]>,
    incoming: Vec<[f64; 2]>,
    paused: bool,
    waveform: Waveform,
    frequency: f64,
    config: SerialConfig,
    ports: Vec<PortInfo>,
    port_error: Option<String>,
    variables: BTreeMap<String, Value>,
    figures: BTreeMap<u32, Figure>,
    active_figure: u32,
    worker: ScriptWorker,
    busy: bool,
    reveal_job_result: bool,
    evaluating: bool,
    input: String,
    history: Vec<String>,
    history_at: usize,
    log: VecDeque<(String, bool)>,
    selected: Option<String>,
    scope_auto_scale: bool,
    auto_scale: bool,
    trigger: bool,
    threshold: f64,
    fft: Arc<dyn nexus_core::rustfft_export::Fft<f64>>,
    fft_buffer: Vec<Complex>,
    fft_scratch: Vec<Complex>,
    hann: Vec<f64>,
    spectrum: Vec<[f64; 2]>,
    spectrum_at: Instant,
    view: View,
    academy: academy::Academy,
    documents: Documents,
    directory: PathBuf,
    directory_input: String,
    files: Vec<FileEntry>,
    file_filter: String,
    browser_selection: Option<PathBuf>,
    selection: String,
    cursor: usize,
    find_open: bool,
    find: String,
    replace: String,
    find_offset: usize,
    history_open: bool,
    history_filter: String,
    variable_filter: String,
    column_start: usize,
    plot_x: usize,
    plot_y: usize,
    data_dirty: Option<String>,
    confirm: Option<Confirm>,
    allow_close: bool,
    status: String,
    recovery_path: Option<PathBuf>,
    recovery_at: Instant,
    recovery_worker: Option<JoinHandle<Result<(), String>>>,
    focus_command: bool,
}
impl NexusApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let preferences = crate::preferences::Preferences::load();
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, &preferences);
        let applied_dark = preferences.dark(&cc.egui_ctx);
        if let Some(state) = &cc.wgpu_render_state {
            let info = state.adapter.get_info();
            eprintln!("NexusLab GPU: {} ({:?})", info.name, info.backend);
        }
        let mut planner = nexus_core::rustfft_export::FftPlanner::new();
        let fft = planner.plan_fft_forward(4096);
        let fft_scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
        let (ports, port_error) = match nexus_hardware::ports() {
            Ok(p) => (p, None),
            Err(e) => (Vec::new(), Some(e.to_string())),
        };
        let recovery_path = documents::recovery_path();
        let recovery = recovery_path
            .as_ref()
            .and_then(|path| nexus_script::workspace::read_limited(path, 128 * 1024 * 1024).ok())
            .and_then(|bytes| serde_json::from_slice::<Recovery>(&bytes).ok());
        let closed_panels: BTreeSet<Panel> = recovery
            .as_ref()
            .filter(|_| preferences.restore_layout)
            .map(|r| {
                r.closed
                    .iter()
                    .filter_map(|key| Panel::from_key(key))
                    .collect()
            })
            .unwrap_or_default();
        let hardware_visible = recovery
            .as_ref()
            .filter(|_| preferences.restore_layout)
            .is_some_and(|r| r.hardware_visible);
        let view = recovery
            .as_ref()
            .filter(|_| preferences.restore_layout)
            .map(|r| match r.view.as_str() {
                "command" => View::Command,
                "figures" => View::Figures,
                "variables" => View::Variables,
                "scope" => View::Scope,
                "tutorial" => View::Tutorial,
                "welcome" => View::Welcome,
                _ => View::Workbench,
            })
            .unwrap_or(View::Workbench);
        let detached: BTreeSet<Panel> = recovery
            .as_ref()
            .filter(|_| preferences.restore_layout)
            .map(|r| {
                r.detached
                    .iter()
                    .filter_map(|name| Panel::from_key(name))
                    .filter(|p| !closed_panels.contains(p))
                    .collect()
            })
            .unwrap_or_default();
        let documents = recovery
            .as_ref()
            .and_then(Documents::restore)
            .unwrap_or_else(Documents::new);
        let directory = recovery
            .as_ref()
            .map(|r| r.directory.clone())
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let history = recovery
            .map(|r| {
                r.history
                    .into_iter()
                    .filter(|s| s.len() <= 65536)
                    .take(500)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let acquisition = Acquisition::start_mock();
        if !detached.contains(&Panel::Scope)
            && (view != View::Scope || closed_panels.contains(&Panel::Scope))
        {
            acquisition.disconnect();
        }
        let mut app = Self {
            preferences,
            preferences_open: false,
            applied_dark,
            closed_panels,
            hardware_visible,
            toolkit: toolkit::Toolkit::default(),
            about_open: false,
            about_diagnostics: false,
            diagnostics: crate::diagnostics::read(),
            diagnostics_at: Instant::now(),
            graphics: cc
                .wgpu_render_state
                .as_ref()
                .map(|s| {
                    let i = s.adapter.get_info();
                    format!("WGPU / {:?} — {}", i.backend, i.name)
                })
                .unwrap_or_else(|| "WGPU unavailable".into()),
            completion: completion::Completion::default(),
            detached,
            themed_viewports: BTreeSet::new(),
            focus_panel: None,
            capture: std::env::var_os("NEXUSLAB_CAPTURE").map(PathBuf::from),
            capture_requested: false,
            capture_at: Instant::now(),
            acquisition,
            samples: VecDeque::with_capacity(32768),
            incoming: Vec::with_capacity(65536),
            paused: false,
            waveform: Waveform::Sine,
            frequency: 1000.0,
            config: SerialConfig::default(),
            ports,
            port_error,
            variables: BTreeMap::from([(
                "pi".into(),
                Value::Real(Matrix::scalar(std::f64::consts::PI)),
            )]),
            figures: BTreeMap::new(),
            active_figure: 1,
            worker: ScriptWorker::new(cc.egui_ctx.clone()),
            busy: false,
            reveal_job_result: true,
            evaluating: false,
            input: String::new(),
            history_at: history.len(),
            history,
            log: VecDeque::new(),
            selected: None,
            scope_auto_scale: true,
            auto_scale: true,
            trigger: false,
            threshold: 0.0,
            fft,
            fft_buffer: vec![Complex::default(); 4096],
            fft_scratch,
            hann: nexus_core::window(Window::Hann, 4096),
            spectrum: Vec::new(),
            spectrum_at: Instant::now(),
            view,
            academy: academy::Academy::default(),
            documents,
            directory_input: directory.to_string_lossy().into_owned(),
            directory,
            files: Vec::new(),
            file_filter: String::new(),
            browser_selection: None,
            selection: String::new(),
            cursor: 0,
            find_open: false,
            find: String::new(),
            replace: String::new(),
            find_offset: 0,
            history_open: false,
            history_filter: String::new(),
            variable_filter: String::new(),
            column_start: 0,
            plot_x: 1,
            plot_y: 2,
            data_dirty: None,
            confirm: None,
            allow_close: false,
            status: "Ready".into(),
            recovery_path,
            recovery_at: Instant::now(),
            recovery_worker: None,
            focus_command: false,
        };
        app.refresh_files();
        app
    }
    fn notify(&mut self, message: String, success: bool) {
        self.status = message
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("Ready")
            .chars()
            .take(160)
            .collect();
        self.log.push_back((message, success));
    }
    fn submit(&mut self, request: Request) {
        if self.busy {
            self.notify("An operation is already running.".into(), false);
            return;
        }
        self.evaluating = matches!(&request, Request::Evaluate { .. });
        let edit = self.pending_edit();
        if let Some((name, value)) = &edit {
            if let Err(e) =
                nexus_script::workspace::validate(&BTreeMap::from([(name.clone(), value.clone())]))
            {
                self.data_dirty = Some(name.clone());
                self.notify(e.to_string(), false);
                return;
            }
        }
        match self.worker.tx.as_ref().unwrap().send(Job { request, edit }) {
            Ok(()) => {
                self.busy = true;
                self.reveal_job_result = true;
                self.status = "Working…".into();
            }
            Err(e) => self.notify(e.to_string(), false),
        }
    }
    fn pending_edit(&mut self) -> Option<(String, Value)> {
        self.data_dirty.take().and_then(|name| {
            self.variables
                .get(&name)
                .cloned()
                .map(|value| (name, value))
        })
    }
    fn run_code(&mut self, code: String, label: String, editor: bool) {
        if self.busy || code.trim().is_empty() {
            return;
        }
        self.log.push_back((
            if editor {
                format!("Running {label}…")
            } else {
                format!(">> {code}")
            },
            true,
        ));
        self.submit(Request::Evaluate {
            code,
            label,
            editor,
        });
    }
    fn execute(&mut self) {
        self.completion.open = false;
        if self.busy || self.input.trim().is_empty() {
            return;
        }
        let code = std::mem::take(&mut self.input);
        self.history.push(code.clone());
        if self.history.len() > 500 {
            self.history.remove(0);
        }
        self.history_at = self.history.len();
        match code.trim() {
            "clc" => {
                self.log.clear();
                self.status = "Command window cleared".into();
            }
            "pwd" => self.notify(self.directory.display().to_string(), true),
            command if command.starts_with("cd ") => {
                let path = command[3..].trim().trim_matches(['\'', '"']);
                self.change_directory(self.resolve_path(path));
            }
            command if command.starts_with("run ") => {
                let path = self.resolve_path(command[4..].trim().trim_matches(['\'', '"']));
                match documents::Document::open(0, &path) {
                    Ok(doc) => self.run_code(doc.source(), path.display().to_string(), false),
                    Err(e) => self.notify(e.to_string(), false),
                }
            }
            _ => self.run_code(code, "Command window".into(), false),
        }
    }
    fn run_editor(&mut self, selection: bool) {
        if self.busy {
            return;
        }
        let doc = self.documents.current();
        let (code, label) = if selection {
            if self.selection.trim().is_empty() {
                self.notify("Select code in the editor first.".into(), false);
                return;
            }
            (
                if doc.rhai {
                    format!("rhai: {}", self.selection)
                } else {
                    self.selection.clone()
                },
                format!("{} (selection)", doc.name),
            )
        } else {
            (doc.source(), doc.name.clone())
        };
        self.run_code(code, label, true);
    }
    fn set_view(&mut self, view: View) {
        if view != View::Welcome {
            self.hardware_visible = view == View::Scope;
        }
        if let Some(panel) = Panel::for_view(view) {
            self.closed_panels.remove(&panel);
            if self.detached.contains(&panel) {
                self.focus_panel = Some(panel);
            }
        }
        if view == View::Scope && self.acquisition.status() == "Disconnected" {
            self.acquisition.mock(self.waveform, self.frequency);
            self.samples.clear();
        }
        if view == View::Scope {
            self.scope_auto_scale = true;
        }
        if view == View::Command {
            self.focus_command = true;
        }
        self.view = view;
        self.auto_scale = true;
    }
    fn poll(&mut self) {
        self.incoming.clear();
        self.acquisition.drain(&mut self.incoming, 65536);
        if !self.paused {
            for &point in &self.incoming {
                if self.samples.len() == 32768 {
                    self.samples.pop_front();
                }
                self.samples.push_back(point);
            }
        }
        while let Ok(result) = self.worker.rx.try_recv() {
            self.variables = result.variables;
            self.figures = result.figures;
            self.busy = false;
            self.evaluating = false;
            self.notify(result.message, result.success);
            if let Some(stats) = result.statistics {
                self.toolkit.statistics = Some(stats);
            }
            if !result.success && self.toolkit.open.is_some() {
                self.toolkit.error = Some(self.status.clone());
            }
            if let Some(name) = result.imported {
                self.selected = Some(name);
                self.column_start = 0;
                if self.reveal_job_result {
                    self.set_view(View::Variables);
                }
            }
            if result.plotted {
                self.active_figure = result.active_figure;
                self.auto_scale = true;
                if !result.editor && self.reveal_job_result {
                    self.set_view(View::Figures);
                }
            }
            if self
                .selected
                .as_ref()
                .is_some_and(|name| !self.variables.contains_key(name))
            {
                self.selected = None;
            }
        }
        while self.log.len() > 500 {
            self.log.pop_front();
        }
        if (self.view == View::Scope || self.detached.contains(&Panel::Scope))
            && !self.paused
            && self.samples.len() >= 4096
            && self.spectrum_at.elapsed() >= Duration::from_millis(100)
        {
            for (i, p) in self
                .samples
                .iter()
                .skip(self.samples.len() - 4096)
                .enumerate()
            {
                self.fft_buffer[i] = Complex::new(p[1] * self.hann[i], 0.0);
            }
            self.fft
                .process_with_scratch(&mut self.fft_buffer, &mut self.fft_scratch);
            self.spectrum = self.fft_buffer[..2048]
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    [
                        i as f64 * self.acquisition.sample_rate() / 4096.0 / 1000.0,
                        20.0 * (c.norm() / 1024.0).max(1e-6).log10(),
                    ]
                })
                .collect();
            self.spectrum_at = Instant::now();
        }
        if self
            .recovery_worker
            .as_ref()
            .is_some_and(|w| w.is_finished())
        {
            if let Ok(Err(e)) = self.recovery_worker.take().unwrap().join() {
                self.notify(format!("Draft recovery: {e}"), false);
            }
        }
        if self.recovery_at.elapsed() > Duration::from_secs(30) && self.recovery_worker.is_none() {
            self.recovery_at = Instant::now();
            if let Some(path) = self.recovery_path.clone() {
                let recovery = self.recovery_snapshot();
                self.recovery_worker =
                    Some(std::thread::spawn(move || save_recovery(&path, &recovery)));
            }
        }
    }
    fn recovery_snapshot(&self) -> Recovery {
        Recovery {
            hardware_visible: self.hardware_visible,
            closed: self.closed_panels.iter().map(|p| p.key().into()).collect(),
            view: Panel::for_view(self.view)
                .map_or("welcome", |p| p.key())
                .into(),
            detached: self.detached.iter().map(|p| p.key().into()).collect(),
            version: 1,
            documents: self.documents.tabs.clone(),
            active: self.documents.active,
            directory: self.directory.clone(),
            history: self.history.clone(),
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.confirm.is_some() || self.about_open || self.preferences_open {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.preferences_open = true;
            return;
        }
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::W,
            )
        }) {
            self.close_all_panels(ctx);
            return;
        }
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::P,
            )
        }) {
            self.restore_panels(ctx);
            return;
        }
        let (new, open, save, save_as, run, selection, find, command, clear) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::N),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::O),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::S),
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::S,
                ),
                i.consume_key(egui::Modifiers::NONE, egui::Key::F5)
                    || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter),
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Enter,
                ),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::F),
                i.consume_key(egui::Modifiers::NONE, egui::Key::F6),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::L),
            )
        });
        if new {
            self.new_document("untitled.nx", "", false);
        }
        if open {
            self.open_dialog();
        }
        if save_as {
            self.save_document(true);
        } else if save {
            self.save_document(false);
        }
        if selection {
            self.run_editor(true);
        } else if run {
            self.run_editor(false);
        }
        if find {
            self.find_open = !self.find_open;
            self.set_view(View::Workbench);
        }
        if command {
            self.set_view(View::Command);
        }
        if clear {
            self.log.clear();
        }
    }
}
fn save_recovery(path: &Path, recovery: &Recovery) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec(recovery).map_err(|e| e.to_string())?;
    nexus_script::workspace::atomic_write(path, &bytes).map_err(|e| e.to_string())
}
impl eframe::App for NexusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.applied_dark != self.preferences.dark(ctx) {
            self.apply_preferences(ctx);
        }
        self.poll();
        self.shortcuts(ctx);
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.documents.tabs.iter().any(|d| d.dirty())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Exit);
        }
        if self.capture.is_some()
            && !self.capture_requested
            && self.capture_at.elapsed() > Duration::from_secs(3)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.capture_requested = true;
        }
        if let Some(path) = self.capture.clone() {
            let screenshot = ctx.input(|i| {
                i.events.iter().find_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(image.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(screenshot) = screenshot {
                let bytes: Vec<u8> = screenshot
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_array())
                    .collect();
                if let Err(e) = image::save_buffer(
                    &path,
                    &bytes,
                    screenshot.size[0] as u32,
                    screenshot.size[1] as u32,
                    image::ColorType::Rgba8,
                ) {
                    eprintln!("Capture failed: {e}");
                }
                self.capture = None;
            }
        }
        self.toolbar(ctx);
        if self.preferences.show_status_bar {
            self.status_bar(ctx);
        }
        if self.view != View::Command && self.panel_docked(Panel::Command) {
            self.command_panel(ctx);
        }
        if self.hardware_visible && self.panel_docked(Panel::Hardware) {
            self.hardware(ctx);
        } else if self.panel_docked(Panel::Files) {
            self.browser(ctx);
        }
        if self.panel_docked(Panel::Workspace) {
            self.inspector(ctx);
        }
        if Panel::for_view(self.view).is_some_and(|p| self.detached.contains(&p)) {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(theme::palette(ctx).background)
                        .inner_margin(8),
                )
                .show(ctx, |ui| {
                    self.detached_placeholder(ui, Panel::for_view(self.view).unwrap())
                });
        } else if self.view == View::Scope {
            self.scope_canvas(ctx);
        } else {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(theme::palette(ctx).background)
                        .inner_margin(8),
                )
                .show(ctx, |ui| match self.view {
                    View::Welcome => self.welcome_ui(ui),
                    View::Workbench => self.editor_ui(ui),
                    View::Figures => self.figures_ui(ui),
                    View::Variables => self.variables_ui(ui),
                    View::Command => self.command_ui(ui),
                    View::Help => self.help_ui(ui),
                    View::Tutorial => self.academy_ui(ui),
                    View::Scope => {}
                });
        }
        self.confirm_ui(ctx);
        self.toolkit_ui(ctx);
        self.preferences_ui(ctx);
        self.about_ui(ctx);
        self.detached_windows(ctx);
        if (self.view == View::Scope || self.detached.contains(&Panel::Scope)) && !self.paused {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
    fn on_exit(&mut self) {
        if let Some(worker) = self.recovery_worker.take() {
            let _ = worker.join();
        }
        if let Some(path) = &self.recovery_path {
            if let Err(e) = save_recovery(path, &self.recovery_snapshot()) {
                eprintln!("Draft recovery failed: {e}");
            }
        }
    }
}
