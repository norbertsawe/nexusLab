use super::*;

impl NexusApp {
    pub(super) fn resolve_path(&self, text: &str) -> PathBuf {
        let path = PathBuf::from(text);
        if path.is_absolute() {
            path
        } else {
            self.directory.join(path)
        }
    }
    pub(super) fn refresh_files(&mut self) {
        match std::fs::read_dir(&self.directory) {
            Ok(entries) => {
                self.files = entries
                    .filter_map(Result::ok)
                    .take(2000)
                    .map(|entry| FileEntry {
                        directory: entry.path().is_dir(),
                        name: entry.file_name().to_string_lossy().into_owned(),
                        path: entry.path(),
                    })
                    .collect();
                self.files.sort_by(|a, b| {
                    b.directory
                        .cmp(&a.directory)
                        .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                });
            }
            Err(e) => self.notify(format!("Cannot read folder: {e}"), false),
        }
    }
    pub(super) fn change_directory(&mut self, path: PathBuf) {
        match path.canonicalize() {
            Ok(path) if path.is_dir() => {
                self.directory = path;
                self.directory_input = self.directory.display().to_string();
                self.refresh_files();
            }
            Ok(_) => self.notify("Choose a folder, not a file.".into(), false),
            Err(e) => self.notify(e.to_string(), false),
        }
    }
    pub(super) fn new_document(&mut self, name: &str, source: &str, rhai: bool) {
        if self.documents.tabs.len() >= 32 {
            self.notify(
                "Close an editor tab before creating another (maximum 32).".into(),
                false,
            );
            return;
        }
        self.documents.add(name, source, rhai);
        self.selection.clear();
        self.cursor = 0;
        self.set_view(View::Workbench);
    }
    pub(super) fn open_path(&mut self, path: PathBuf) {
        self.browser_selection = Some(path.clone());
        if path.is_dir() {
            self.change_directory(path);
            return;
        }
        match path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "csv" => {
                let name = self.import_name(&path);
                self.submit(Request::ImportCsv { path, name });
            }
            "nxw" => self.submit(Request::LoadWorkspace(path)),
            _ => match self.documents.open(&path) {
                Ok(()) => {
                    self.selection.clear();
                    self.cursor = 0;
                    self.set_view(View::Workbench);
                }
                Err(e) => self.notify(e.to_string(), false),
            },
        }
    }
    pub(super) fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open a script, dataset, or workspace")
            .set_directory(&self.directory)
            .add_filter("NexusLab files", &["nx", "m", "rhai", "csv", "nxw", "txt"])
            .add_filter("All files", &["*"])
            .pick_file()
        {
            self.open_path(path);
        }
    }
    pub(super) fn folder_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Open working folder")
            .set_directory(&self.directory)
            .pick_folder()
        {
            self.change_directory(path);
        }
    }
    pub(super) fn save_document(&mut self, save_as: bool) -> bool {
        let doc = self.documents.current();
        let needs_path = save_as
            || doc.path.is_none()
            || doc.rhai
                && doc
                    .path
                    .as_ref()
                    .is_some_and(|p| p.extension().is_none_or(|e| e != "rhai"));
        let path = if needs_path {
            let name = if doc.rhai {
                format!(
                    "{}.rhai",
                    Path::new(&doc.name)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                )
            } else {
                doc.name.clone()
            };
            let dialog = rfd::FileDialog::new()
                .set_title("Save script")
                .set_directory(&self.directory)
                .set_file_name(name);
            let dialog = if doc.rhai {
                dialog.add_filter("Rhai scripts", &["rhai"])
            } else {
                dialog.add_filter("NexusLab scripts", &["nx", "m", "txt"])
            };
            let Some(path) = dialog.save_file() else {
                return false;
            };
            path
        } else {
            doc.path.clone().unwrap()
        };
        match self
            .documents
            .current_mut()
            .save_to(path.clone(), needs_path)
        {
            Ok(()) => {
                self.notify(format!("Saved {}", path.display()), true);
                self.refresh_files();
                true
            }
            Err(e) => {
                self.notify(e.to_string(), false);
                false
            }
        }
    }
    pub(super) fn request_close_tab(&mut self, index: usize) {
        if self.documents.tabs[index].dirty() {
            self.confirm = Some(Confirm::CloseTab(index));
        } else {
            let last = self.documents.tabs.len() == 1;
            self.documents.close(index);
            if last {
                self.closed_panels.insert(Panel::Editor);
                self.view = View::Welcome;
            }
            self.selection.clear();
        }
    }
    pub(super) fn save_workspace(&mut self) {
        if self.busy {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.directory)
            .set_title("Save numerical workspace")
            .set_file_name("workspace.nxw")
            .add_filter("NexusLab workspace", &["nxw"])
            .save_file()
        {
            self.submit(Request::SaveWorkspace(path));
        }
    }
    pub(super) fn load_workspace(&mut self) {
        if self.busy {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.directory)
            .set_title("Load workspace — replaces current variables")
            .add_filter("NexusLab workspace", &["nxw"])
            .pick_file()
        {
            self.data_dirty = None;
            self.submit(Request::LoadWorkspace(path));
        }
    }
    fn import_name(&self, path: &Path) -> String {
        let base: String = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let base = if base.is_empty() {
            "data".into()
        } else if base.chars().next().is_some_and(|c| c.is_ascii_digit()) || base == "pi" {
            format!("data_{base}")
        } else {
            base
        };
        let mut name = base.clone();
        let mut index = 1;
        while self.variables.contains_key(&name) {
            name = format!("{base}_{index}");
            index += 1;
        }
        name
    }
    pub(super) fn import_csv(&mut self) {
        if self.busy {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.directory)
            .set_title("Import numeric CSV")
            .add_filter("CSV data", &["csv"])
            .pick_file()
        {
            let name = self.import_name(&path);
            self.submit(Request::ImportCsv { path, name });
        }
    }
    pub(super) fn export_csv(&mut self) {
        if self.busy {
            return;
        }
        let Some(name) = self.selected.clone() else {
            self.notify("Select a workspace variable to export.".into(), false);
            return;
        };
        let Some(value) = self.variables.get(&name).cloned() else {
            return;
        };
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.directory)
            .set_title("Export selected variable")
            .set_file_name(format!("{name}.csv"))
            .add_filter("CSV data", &["csv"])
            .save_file()
        {
            self.submit(Request::ExportCsv(path, value));
        }
    }
    pub(super) fn export_figure(&mut self, format: super::figures::ExportFormat) {
        if self.busy {
            return;
        }
        let Some(figure) = self
            .figures
            .get(&self.active_figure)
            .or_else(|| self.figures.values().next())
            .cloned()
        else {
            return;
        };
        let (extension, label) = match format {
            super::figures::ExportFormat::Svg => ("svg", "SVG vector figure"),
            super::figures::ExportFormat::Png => ("png", "PNG high-DPI image"),
            super::figures::ExportFormat::Csv => ("csv", "CSV raw data"),
        };
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.directory)
            .set_title("Export figure")
            .set_file_name(format!("figure-{}.{extension}", figure.id))
            .add_filter(label, &[extension])
            .save_file()
        {
            self.submit(Request::ExportFigure(path, figure, format));
        }
    }
    pub(super) fn export_log(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_directory(&self.directory)
            .set_title("Save command window transcript")
            .set_file_name("command-window.txt")
            .add_filter("Text", &["txt"])
            .save_file()
        {
            let text = self
                .log
                .iter()
                .map(|(text, _)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            match nexus_script::workspace::atomic_write(&path, text.as_bytes()) {
                Ok(()) => self.notify(format!("Transcript saved to {}", path.display()), true),
                Err(e) => self.notify(e.to_string(), false),
            }
        }
    }
    pub(super) fn confirm_ui(&mut self, ctx: &egui::Context) {
        let Some(confirm) = &self.confirm else {
            return;
        };
        let (exit, index) = match confirm {
            Confirm::Exit => (true, None),
            Confirm::CloseTab(i) => (false, Some(*i)),
        };
        let mut action = 0;
        egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
            ui.heading("Unsaved changes");
            if exit {
                ui.label("Save your modified scripts before leaving?");
                ui.label(
                    RichText::new(
                        "Keep drafts stores unsaved editor contents for your next session.",
                    )
                    .color(theme::palette(ui.ctx()).muted),
                );
            } else {
                ui.label(format!(
                    "Save changes to {}?",
                    self.documents.tabs[index.unwrap()].name
                ));
            }
            ui.horizontal(|ui| {
                if ui
                    .button(if exit { "Save all and exit" } else { "Save" })
                    .clicked()
                {
                    action = 1;
                }
                if ui
                    .button(if exit {
                        "Keep drafts and exit"
                    } else {
                        "Discard changes"
                    })
                    .clicked()
                {
                    action = 2;
                }
                if ui.button("Cancel").clicked() {
                    action = 3;
                }
            });
        });
        if action == 0 {
            return;
        }
        if action == 3 {
            self.confirm = None;
            return;
        }
        if exit {
            if action == 1 {
                let current = self.documents.active;
                for i in 0..self.documents.tabs.len() {
                    if self.documents.tabs[i].dirty() {
                        self.documents.active = i;
                        if !self.save_document(false) {
                            self.documents.active = current;
                            return;
                        }
                    }
                }
                self.documents.active = current;
            }
            self.confirm = None;
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            let index = index.unwrap();
            if action == 1 {
                let current = self.documents.active;
                self.documents.active = index;
                if !self.save_document(false) {
                    self.documents.active = current;
                    return;
                }
            }
            let last = self.documents.tabs.len() == 1;
            self.documents.close(index);
            if last {
                self.closed_panels.insert(Panel::Editor);
                self.view = View::Welcome;
            }
            self.confirm = None;
            self.selection.clear();
        }
    }
}
