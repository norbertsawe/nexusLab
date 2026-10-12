use super::*;

impl NexusApp {
    pub(super) fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("workbench_toolbar")
            .frame(egui::Frame::new().fill(theme::palette(ctx).background).inner_margin(egui::Margin::symmetric(6, 3)))
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New script     Ctrl+N").clicked() { self.new_document("untitled.nx", "", false); ui.close_menu(); }
                        if ui.button("Open...          Ctrl+O").clicked() { self.open_dialog(); ui.close_menu(); }
                        if ui.button("Open folder...").clicked() { self.folder_dialog(); ui.close_menu(); }
                        if ui.button("Save           Ctrl+S").clicked() { self.save_document(false); ui.close_menu(); }
                        if ui.button("Save As...       Ctrl+Shift+S").clicked() { self.save_document(true); ui.close_menu(); }
                        ui.separator();
                        if ui.add_enabled(!self.busy, egui::Button::new("Save workspace...")).clicked() { self.save_workspace(); ui.close_menu(); }
                        if ui.add_enabled(!self.busy, egui::Button::new("Load workspace...")).clicked() { self.load_workspace(); ui.close_menu(); }
                        if ui.add_enabled(!self.busy, egui::Button::new("Import CSV...")).clicked() { self.import_csv(); ui.close_menu(); }
                        if ui.add_enabled(self.selected.is_some() && !self.busy, egui::Button::new("Export variable as CSV...")).clicked() { self.export_csv(); ui.close_menu(); }
                        if ui.button("Save command transcript...").clicked() { self.export_log(); ui.close_menu(); }
                        ui.separator();
                        if ui.button("Exit").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); ui.close_menu(); }
                    });
                    ui.menu_button("Edit", |ui| {
                        if ui.button("Find and replace     Ctrl+F").clicked() { self.find_open = true; self.set_view(View::Workbench); ui.close_menu(); }
                        if ui.button("Clear command window     Ctrl+L").clicked() { self.log.clear(); ui.close_menu(); }
                        ui.separator();
                        if ui.button("Preferences...     Ctrl+,").clicked() { self.preferences_open = true; ui.close_menu(); }
                    });
                    ui.menu_button("View", |ui| {
                        for (view, title) in [(View::Workbench,"Script Editor"), (View::Command,"Command Window"), (View::Figures,"Figures"), (View::Variables,"Variable Editor"), (View::Scope,"Live Scope")] {
                            if ui.selectable_label(self.view == view, title).clicked() { self.set_view(view); ui.close_menu(); }
                        }
                        ui.separator();
                        for panel in [Panel::Files, Panel::Workspace, Panel::Hardware] {
                            let mut visible = self.detached.contains(&panel) || (self.panel_docked(panel) && match panel {
                                Panel::Hardware => self.hardware_visible,
                                Panel::Files => !self.hardware_visible || !self.panel_docked(Panel::Hardware),
                                _ => true,
                            });
                            if ui.checkbox(&mut visible, panel.title()).changed() {
                                if visible { self.open_panel(panel); } else { self.close_panel(ctx,panel); }
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("Close all panels     Ctrl+Shift+W").clicked() { self.close_all_panels(ctx);ui.close_menu(); }
                        if ui.button("Restore panels     Ctrl+Shift+P").clicked() { self.restore_panels(ctx);ui.close_menu(); }
                    });
                    ui.menu_button("Run", |ui| {
                        if ui.add_enabled(!self.busy, egui::Button::new("Run script     F5")).clicked() { self.run_editor(false); ui.close_menu(); }
                        if ui.add_enabled(!self.busy && !self.selection.trim().is_empty(), egui::Button::new("Run selection     Ctrl+Shift+Enter")).clicked() { self.run_editor(true); ui.close_menu(); }
                        if ui.add_enabled(self.busy && self.evaluating, egui::Button::new("Stop")).clicked() { self.worker.cancel.store(true, Ordering::Relaxed); ui.close_menu(); }
                    });
                    ui.menu_button("Tools", |ui| {
                        if ui.button("Filter Design...").clicked() { self.open_tool(toolkit::Tool::Filter);ui.close_menu(); }
                        if ui.button("Spectral Analyzer...").clicked() { self.open_tool(toolkit::Tool::Spectral);ui.close_menu(); }
                        if ui.button("Statistics Summary...").clicked() { self.open_tool(toolkit::Tool::Statistics);ui.close_menu(); }
                        ui.separator();
                        ui.menu_button("New from template", |ui| {
                            if ui.button("Regression and statistics").clicked() { self.new_document("analysis.nx", documents::ANALYSIS, false); ui.close_menu(); }
                            if ui.button("Matrices and linear systems").clicked() { self.new_document("matrices.m", documents::MATRICES, false); ui.close_menu(); }
                            if ui.button("Digital filtering").clicked() { self.new_document("filtering.nx", documents::FILTERING, false); ui.close_menu(); }
                            if ui.button("Rhai loops and conditionals").clicked() { self.new_document("experiment.rhai", "let total = 0;\nfor n in 0..100 { total += n; }\nprint(total);\ntotal", true); ui.close_menu(); }
                        });
                    });
                    ui.menu_button("Window", |ui| {
                        for panel in Panel::all() {
                            if ui.button(format!("{} {}", if self.closed_panels.contains(&panel) { "Open" } else if self.detached.contains(&panel) { "Dock" } else { "Detach" }, panel.title())).clicked() {
                                if self.closed_panels.contains(&panel) { self.open_panel(panel); } else if self.detached.contains(&panel) { self.dock_panel(ctx, panel); } else { self.detach_panel(panel); }
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        if ui.button("Dock all panels").clicked() { for panel in self.detached.clone() { self.dock_panel(ctx, panel); } ui.close_menu(); }
                        if ui.button("Close all panels").clicked() { self.close_all_panels(ctx);ui.close_menu(); }
                    });
                    ui.menu_button("Help", |ui| {
                        if ui.button("About NexusLab").clicked() { self.about_open=true;ui.close_menu(); }
                        if ui.button("Commands and keyboard shortcuts").clicked() { self.set_view(View::Help); ui.close_menu(); }
                    });
                });
                if self.preferences.show_toolbar && self.workbench_visible() {
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("New").clicked() { self.new_document("untitled.nx", "", false); }
                    if ui.button("Open").clicked() { self.open_dialog(); }
                    if ui.button("Save").clicked() { self.save_document(false); }
                    ui.separator();
                    if ui.add_enabled(!self.busy, egui::Button::new("Run  F5")).clicked() { self.run_editor(false); }
                    if ui.add_enabled(self.busy && self.evaluating, egui::Button::new("Stop")).clicked() { self.worker.cancel.store(true, Ordering::Relaxed); self.status = "Cancelling...".into(); }
                });
                }
                if self.workbench_visible() {
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    for (view, title) in [(View::Workbench,"Editor"), (View::Command,"Command Window"), (View::Figures,"Figures"), (View::Variables,"Variable Editor"), (View::Scope,"Live Scope")] {
                        if Panel::for_view(view).is_some_and(|p|self.closed_panels.contains(&p)) { continue; }
                        let label = if view == View::Figures { format!("{title} ({})", self.figures.len()) } else { title.into() };
                        if ui.selectable_label(self.view == view, label).clicked() { self.set_view(view); }
                    }
                });
                }
            });
    }
    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("workbench_status")
            .frame(
                egui::Frame::new()
                    .fill(theme::palette(ctx).background)
                    .inner_margin(egui::Margin::symmetric(6, 3)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let connection = self.acquisition.status();
                    let offline = connection == "Disconnected";
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        rect.center(),
                        3.0,
                        if offline {
                            egui::Color32::GRAY
                        } else if connection.to_lowercase().contains("mock") {
                            BLUE
                        } else if connection.to_lowercase().contains("connecting")
                            || connection.to_lowercase().contains("error")
                        {
                            egui::Color32::from_rgb(180, 120, 0)
                        } else {
                            GREEN
                        },
                    );
                    ui.label(if offline {
                        "Offline".into()
                    } else {
                        connection
                    });
                    ui.separator();
                    let stats = format!(
                        "{} variables | {:.1} KiB",
                        self.variables.len(),
                        self.variables.values().map(Value::bytes).sum::<usize>() as f64 / 1024.0
                    );
                    let reserved = 190.0;
                    ui.add_sized(
                        [(ui.available_width() - reserved).max(30.0), 20.0],
                        egui::Label::new(RichText::new(&self.status).small()).truncate(),
                    );
                    ui.separator();
                    ui.label(RichText::new(stats).small());
                });
            });
    }
    pub(super) fn browser(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("file_browser")
            .default_width(225.0)
            .width_range(180.0..=380.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::palette(ctx).background)
                    .inner_margin(8),
            )
            .show(ctx, |ui| self.browser_ui(ui));
    }
    pub(super) fn browser_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            theme::label(ui, "Current Folder");
            self.panel_button(ui, Panel::Files);
            if crate::icons::button(
                ui,
                crate::icons::Icon::Folder,
                "Choose current folder",
                true,
            )
            .clicked()
            {
                self.folder_dialog();
            }
            if crate::icons::button(
                ui,
                crate::icons::Icon::Refresh,
                "Refresh current folder",
                true,
            )
            .clicked()
            {
                self.refresh_files();
            }
        });
        let path = ui.add(
            egui::TextEdit::singleline(&mut self.directory_input)
                .desired_width(ui.available_width()),
        );
        if path.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.change_directory(self.resolve_path(&self.directory_input));
        }
        ui.horizontal(|ui| {
            if crate::icons::button(ui, crate::icons::Icon::Up, "Parent folder", true).clicked() {
                if let Some(parent) = self.directory.parent() {
                    self.change_directory(parent.to_path_buf());
                }
            }
            if crate::icons::button(
                ui,
                crate::icons::Icon::Folder,
                "Open script, data or workspace",
                true,
            )
            .clicked()
            {
                self.open_dialog();
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.file_filter)
                .hint_text("Filter files…")
                .desired_width(ui.available_width()),
        );
        ui.add_space(4.0);
        let mut open = None;
        let filter = self.file_filter.to_lowercase();
        let selected_path =
            self.browser_selection
                .as_ref()
                .or(self.documents.current().path.as_ref());
        let mut visible_count = 0;
        egui::Frame::new()
            .fill(theme::palette(ui.ctx()).surface)
            .stroke(egui::Stroke::new(1.0_f32, theme::palette(ui.ctx()).border))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("files_scroll")
                    .auto_shrink([false, true])
                    .max_height((ui.available_height() - 80.0).max(80.0))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for entry in self
                            .files
                            .iter()
                            .filter(|entry| entry.name.to_lowercase().contains(&filter))
                        {
                            let response = ui
                                .push_id(&entry.path, |ui| {
                                    theme::explorer_row(
                                        ui,
                                        &entry.name,
                                        theme::FileIcon::for_path(&entry.path, entry.directory),
                                        visible_count,
                                        selected_path == Some(&entry.path),
                                    )
                                })
                                .inner;
                            if response
                                .on_hover_text(entry.path.display().to_string())
                                .clicked()
                            {
                                open = Some(entry.path.clone());
                            }
                            visible_count += 1;
                        }
                        if visible_count == 0 {
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new("No matching files")
                                    .color(theme::palette(ui.ctx()).muted),
                            );
                            ui.add_space(8.0);
                        }
                    });
            });
        ui.add_space(4.0);
        ui.label(
            RichText::new(format!("{visible_count} items"))
                .small()
                .color(theme::palette(ui.ctx()).muted),
        );
        if let Some(path) = open {
            self.open_path(path);
        }
    }
    pub(super) fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("workspace_inspector")
            .default_width(240.0)
            .width_range(200.0..=420.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::palette(ctx).background)
                    .inner_margin(8),
            )
            .show(ctx, |ui| self.inspector_ui(ui));
    }
    pub(super) fn inspector_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            theme::label(ui, "Workspace");
            self.panel_button(ui, Panel::Workspace);
        });
        ui.horizontal(|ui| {
            if crate::icons::button(ui, crate::icons::Icon::Save, "Save workspace", !self.busy)
                .clicked()
            {
                self.save_workspace();
            }
            if crate::icons::button(ui, crate::icons::Icon::Folder, "Load workspace", !self.busy)
                .clicked()
            {
                self.load_workspace();
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.variable_filter)
                .hint_text("Filter variables…")
                .desired_width(ui.available_width()),
        );
        let mut clicked = None;
        let name_width = (ui.available_width() - 96.0).clamp(64.0, 220.0);
        egui::ScrollArea::vertical()
            .id_salt("workspace_list")
            .max_height((ui.available_height() - 145.0).max(80.0))
            .show(ui, |ui| {
                egui::Grid::new("workspace_table")
                    .num_columns(3)
                    .striped(true)
                    .spacing(egui::vec2(8.0, 4.0))
                    .show(ui, |ui| {
                        for heading in ["Name", "Type", "Size"] {
                            ui.label(RichText::new(heading).strong());
                        }
                        ui.end_row();
                        for (name, value) in &self.variables {
                            if !name
                                .to_lowercase()
                                .contains(&self.variable_filter.to_lowercase())
                            {
                                continue;
                            }
                            let (rows, cols) = value.shape();
                            let kind = match value {
                                Value::Real(_) => "f64",
                                Value::Complex(_) => "c128",
                                Value::Text(_) => "text",
                            };
                            if ui
                                .add_sized(
                                    [name_width, 24.0],
                                    egui::Button::new(RichText::new(name).monospace())
                                        .selected(self.selected.as_ref() == Some(name))
                                        .frame(false)
                                        .truncate(),
                                )
                                .on_hover_text(name)
                                .clicked()
                            {
                                clicked = Some(name.clone());
                            }
                            ui.label(kind);
                            ui.label(format!("{rows} × {cols}"))
                                .on_hover_text(format!("{} bytes", value.bytes()));
                            ui.end_row();
                        }
                    });
            });
        if let Some(name) = clicked {
            if self.data_dirty.as_ref() != Some(&name) {
                if let Some((old, value)) = self.pending_edit() {
                    self.submit(Request::Put(old, value));
                }
            }
            self.selected = Some(name);
            self.column_start = 0;
            self.set_view(View::Variables);
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if crate::icons::button(ui, crate::icons::Icon::Folder, "Import CSV", !self.busy)
                .clicked()
            {
                self.import_csv();
            }
            if crate::icons::button(
                ui,
                crate::icons::Icon::Save,
                "Export selected variable as CSV",
                !self.busy && self.selected.is_some(),
            )
            .clicked()
            {
                self.export_csv();
            }
        });
        if crate::icons::button(ui, crate::icons::Icon::Trash, "Clear workspace", !self.busy)
            .clicked()
        {
            self.run_code("clear".into(), "Workspace".into(), false);
        }
    }
    pub(super) fn command_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("command_window")
            .resizable(true)
            .default_height(190.0)
            .min_height(110.0)
            .max_height(ctx.screen_rect().height() * 0.75)
            .frame(
                egui::Frame::new()
                    .fill(theme::palette(ctx).surface)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ctx, |ui| self.command_ui(ui));
    }
    pub(super) fn command_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Command Window")
                    .strong()
                    .color(theme::palette(ui.ctx()).text),
            );
            self.panel_button(ui, Panel::Command);
            if self.busy {
                ui.spinner();
                ui.label(RichText::new("Running…").color(BLUE));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.detached.contains(&Panel::Command)
                    && crate::icons::button(
                        ui,
                        crate::icons::Icon::Expand,
                        if self.view == View::Command {
                            "Collapse command window"
                        } else {
                            "Expand command window (F6)"
                        },
                        true,
                    )
                    .clicked()
                {
                    self.set_view(if self.view == View::Command {
                        View::Workbench
                    } else {
                        View::Command
                    });
                }
                if crate::icons::button(
                    ui,
                    crate::icons::Icon::Save,
                    "Save command transcript",
                    true,
                )
                .clicked()
                {
                    self.export_log();
                }
                if crate::icons::button(
                    ui,
                    crate::icons::Icon::Trash,
                    "Clear command window (Ctrl+L)",
                    true,
                )
                .clicked()
                {
                    self.log.clear();
                }
                if crate::icons::button(
                    ui,
                    crate::icons::Icon::History,
                    "Show command history",
                    true,
                )
                .clicked()
                {
                    self.history_open = !self.history_open;
                }
            });
        });
        let height = (ui.available_height() - 52.0).max(40.0);
        if self.history_open {
            ui.add(
                egui::TextEdit::singleline(&mut self.history_filter)
                    .hint_text("Search command history…"),
            );
            let mut chosen = None;
            egui::ScrollArea::vertical()
                .id_salt("history_scroll")
                .max_height((height - 38.0).max(30.0))
                .min_scrolled_height((height - 38.0).max(30.0))
                .show(ui, |ui| {
                    for entry in self.history.iter().rev().filter(|s| {
                        s.to_lowercase()
                            .contains(&self.history_filter.to_lowercase())
                    }) {
                        let response = ui.selectable_label(false, RichText::new(entry).monospace());
                        if response.clicked() {
                            chosen = Some((entry.clone(), response.double_clicked()));
                        }
                    }
                });
            if let Some((code, run)) = chosen {
                self.input = code;
                self.focus_command = true;
                if run {
                    self.execute();
                }
            }
        } else {
            egui::ScrollArea::vertical()
                .id_salt("command_output")
                .stick_to_bottom(true)
                .max_height(height)
                .min_scrolled_height(height)
                .show(ui, |ui| {
                    for (message, success) in &self.log {
                        ui.add(
                            egui::Label::new(RichText::new(message).monospace().color(
                                if *success {
                                    theme::palette(ui.ctx()).text
                                } else {
                                    theme::palette(ui.ctx()).error
                                },
                            ))
                            .selectable(true),
                        );
                        ui.add_space(4.0);
                    }
                });
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(">>").monospace().color(BLUE));
            let prompt_id = ui.make_persistent_id("command_prompt");
            let focused = ui.memory(|m| m.has_focus(prompt_id));
            let tab =
                focused && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab));
            if focused {
                if let Some(state) = egui::TextEdit::load_state(ui.ctx(), prompt_id) {
                    if let Some(range) = state.cursor.char_range() {
                        self.completion.cursor_byte = self
                            .input
                            .char_indices()
                            .nth(range.primary.index)
                            .map_or(self.input.len(), |(i, _)| i);
                    }
                }
                if self.completion.open {
                    let (up, down, enter, escape) = ui.input_mut(|i| {
                        (
                            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                        )
                    });
                    if up {
                        self.completion.index = self.completion.index.saturating_sub(1);
                    }
                    if down {
                        self.completion.index = (self.completion.index + 1)
                            .min(self.completion.candidates.len().saturating_sub(1));
                    }
                    if enter {
                        self.accept_completion();
                    }
                    if escape {
                        self.completion.open = false;
                    }
                }
            }
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .id(prompt_id)
                    .lock_focus(true)
                    .font(egui::TextStyle::Monospace)
                    .desired_width((ui.available_width() - 102.0).max(80.0))
                    .margin(egui::vec2(6.0, 4.0))
                    .hint_text("Enter a command, expression, or help…"),
            );
            if self.focus_command {
                response.request_focus();
                self.focus_command = false;
            }
            if !self.completion.cursor_to_end {
                if let Some(state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                    if let Some(range) = state.cursor.char_range() {
                        self.completion.cursor_byte = self
                            .input
                            .char_indices()
                            .nth(range.primary.index)
                            .map_or(self.input.len(), |(i, _)| i);
                    }
                }
            }
            if tab {
                self.completion_candidates();
            }
            if self.completion.cursor_to_end {
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), response.id) {
                    let index = self.input[..self.completion.cursor_byte.min(self.input.len())]
                        .chars()
                        .count();
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(index),
                        )));
                    state.store(ui.ctx(), response.id);
                }
                self.completion.cursor_to_end = false;
            }
            if response.has_focus() && !self.completion.open {
                if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !self.history.is_empty() {
                    self.history_at = self.history_at.saturating_sub(1);
                    self.input = self.history[self.history_at].clone();
                    self.completion.cursor_byte = self.input.len();
                    self.completion.cursor_to_end = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                    self.history_at = (self.history_at + 1).min(self.history.len());
                    self.input = self
                        .history
                        .get(self.history_at)
                        .cloned()
                        .unwrap_or_default();
                    self.completion.cursor_byte = self.input.len();
                    self.completion.cursor_to_end = true;
                }
            }
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui
                .add_enabled(!self.busy, egui::Button::new("Evaluate"))
                .clicked()
                || enter
            {
                self.execute();
                self.completion.open = false;
                response.request_focus();
            }
            self.completion_popup(ui, response.rect);
        });
    }
    pub(super) fn variables_ui(&mut self, ui: &mut egui::Ui) {
        if self.busy {
            ui.disable();
        }
        ui.horizontal(|ui| {
            ui.heading("Variable Editor");
            self.panel_button(ui, Panel::Variables);
        });
        let Some(name) = self
            .selected
            .clone()
            .filter(|name| self.variables.contains_key(name))
        else {
            ui.label(
                "Select a workspace variable in the panel on the right, or import a CSV dataset.",
            );
            if ui.button("Import CSV…").clicked() {
                self.import_csv();
            }
            return;
        };
        let mut plot = false;
        let mut apply = false;
        let mut delete = false;
        let (rows, cols) = self.variables[&name].shape();
        ui.horizontal(|ui| {
            ui.label(RichText::new(&name).monospace().size(13.0).strong());
            ui.label(
                RichText::new(format!("{rows} × {cols}")).color(theme::palette(ui.ctx()).muted),
            );
            if crate::icons::button(ui, crate::icons::Icon::Save, "Export CSV", !self.busy)
                .clicked()
            {
                self.export_csv();
            }
            apply = crate::icons::button(
                ui,
                crate::icons::Icon::Apply,
                "Apply variable edits",
                !self.busy && self.data_dirty.as_ref() == Some(&name),
            )
            .clicked();
            delete = crate::icons::button(
                ui,
                crate::icons::Icon::Trash,
                "Delete variable",
                !self.busy && name != "pi",
            )
            .clicked();
        });
        ui.horizontal(|ui| {
            if cols > 1 && rows > 1 {
                ui.label("X column");
                ui.add(egui::DragValue::new(&mut self.plot_x).range(1..=cols));
                ui.label("Y column");
                ui.add(egui::DragValue::new(&mut self.plot_y).range(1..=cols));
            }
            plot = crate::icons::button(
                ui,
                crate::icons::Icon::Plot,
                "Plot variable data",
                !self.busy && !matches!(self.variables[&name], Value::Text(_)),
            )
            .clicked();
            if cols > 8 {
                ui.label("First visible column");
                ui.add(
                    egui::DragValue::new(&mut self.column_start).range(0..=cols.saturating_sub(8)),
                );
            }
        });
        if self.data_dirty.as_ref() == Some(&name) {
            ui.label(
                RichText::new("Edited values are applied when you press Apply edits or run code.")
                    .small()
                    .color(BLUE),
            );
        }
        ui.add_space(6.0);
        let mut changed = false;
        match self.variables.get_mut(&name).unwrap() {
            Value::Real(matrix) => {
                let end = (self.column_start + 8).min(matrix.cols);
                egui::ScrollArea::both()
                    .id_salt(("data_grid", &name))
                    .auto_shrink([false; 2])
                    .show_rows(ui, 28.0, matrix.rows, |ui, range| {
                        ui.horizontal(|ui| {
                            ui.add_sized([42.0, 22.0], egui::Label::new("Row"));
                            for c in self.column_start..end {
                                ui.add_sized(
                                    [104.0, 22.0],
                                    egui::Label::new(
                                        RichText::new(format!("{}", c + 1))
                                            .strong()
                                            .color(theme::palette(ui.ctx()).muted),
                                    ),
                                );
                            }
                        });
                        for r in range {
                            ui.push_id(r, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        [42.0, 24.0],
                                        egui::Label::new(
                                            RichText::new(format!("{}", r + 1))
                                                .color(theme::palette(ui.ctx()).muted),
                                        ),
                                    );
                                    for c in self.column_start..end {
                                        changed |= ui
                                            .add_enabled_ui(!self.busy && name != "pi", |ui| {
                                                ui.add_sized(
                                                    [104.0, 24.0],
                                                    egui::DragValue::new(
                                                        &mut matrix.data[r * matrix.cols + c],
                                                    )
                                                    .speed(0.01)
                                                    .max_decimals(8),
                                                )
                                            })
                                            .inner
                                            .changed();
                                    }
                                })
                            });
                        }
                    });
            }
            Value::Complex(values) => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show_rows(ui, 25.0, values.len(), |ui, range| {
                        for i in range {
                            ui.label(
                                RichText::new(format!(
                                    "{:>6}   {:>16.8}  {:+16.8} i",
                                    i + 1,
                                    values[i][0],
                                    values[i][1]
                                ))
                                .monospace(),
                            );
                        }
                    });
            }
            Value::Text(text) => {
                ui.add(
                    egui::TextEdit::multiline(text)
                        .interactive(false)
                        .desired_width(f32::INFINITY)
                        .desired_rows(12),
                );
            }
        }
        if changed {
            self.data_dirty = Some(name.clone());
        }
        if apply {
            if let Some((name, value)) = self.pending_edit() {
                self.submit(Request::Put(name, value));
            }
        }
        if delete {
            self.data_dirty = None;
            self.submit(Request::Delete(name.clone()));
        }
        if plot {
            let code = if matches!(self.variables[&name], Value::Complex(_)) {
                format!("plot(abs({name}))")
            } else if rows > 1 && cols > 1 {
                format!(
                    "plot(column({name},{}),column({name},{}))",
                    self.plot_x.min(cols),
                    self.plot_y.min(cols)
                )
            } else {
                format!("plot({name})")
            };
            self.run_code(code, "Variable Editor".into(), false);
        }
    }
    pub(super) fn help_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("NexusLab Help");
        egui::ScrollArea::vertical().show(ui,|ui| {
            ui.label("Write a script in the editor, run it with F5, and inspect the results in the workspace. Figures are created by your code."); ui.add_space(12.0);
            theme::group(ui).show(ui,|ui| { ui.label(RichText::new(nexus_script::HELP).monospace()); });
            ui.add_space(12.0); ui.label(RichText::new("Keyboard shortcuts").strong());
            for (keys,description) in [("Ctrl+N / Ctrl+O","New / open script"),("Ctrl+S / Ctrl+Shift+S","Save / save as"),("F5 / Ctrl+Enter","Run active script"),("Ctrl+Shift+Enter","Run selected text"),("Ctrl+F","Find / replace in editor"),("F6","Expand command window"),("Ctrl+L","Clear command window"),("Up / Down in command prompt","Recall command history")] { ui.horizontal(|ui| { ui.label(RichText::new(keys).monospace().color(BLUE)); ui.label(description); }); }
            ui.add_space(12.0); ui.label("Workspaces use .nxw files. CSV imports accept rectangular finite numeric data and one optional header row. Save and load preserve real arrays, complex arrays, and text variables.");
            ui.label("Rhai provides loops and conditionals. The MATLAB-style interpreter supports the documented subset; it does not implement MATLAB toolboxes or a graphical debugger.");
            ui.label("Unsaved editor drafts and command history recover automatically. Save your numerical workspace separately before exiting.");
        });
    }
}
