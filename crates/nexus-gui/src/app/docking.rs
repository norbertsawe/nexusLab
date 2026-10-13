use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Panel {
    Editor,
    Command,
    Figures,
    Variables,
    Scope,
    Workspace,
    Files,
    Hardware,
    Tutorial,
}
impl Panel {
    pub fn all() -> [Self; 9] {
        [
            Self::Editor,
            Self::Command,
            Self::Figures,
            Self::Variables,
            Self::Scope,
            Self::Workspace,
            Self::Files,
            Self::Hardware,
            Self::Tutorial,
        ]
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Editor => "Script Editor",
            Self::Command => "Command Window",
            Self::Figures => "Figures",
            Self::Variables => "Variable Editor",
            Self::Scope => "Live Scope",
            Self::Workspace => "Workspace",
            Self::Files => "Current Folder",
            Self::Hardware => "Hardware Manager",
            Self::Tutorial => "Tutorial",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Editor => "editor",
            Self::Command => "command",
            Self::Figures => "figures",
            Self::Variables => "variables",
            Self::Scope => "scope",
            Self::Workspace => "workspace",
            Self::Files => "files",
            Self::Hardware => "hardware",
            Self::Tutorial => "tutorial",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::all().into_iter().find(|panel| panel.key() == key)
    }
    pub fn for_view(view: View) -> Option<Self> {
        match view {
            View::Workbench => Some(Self::Editor),
            View::Command => Some(Self::Command),
            View::Figures => Some(Self::Figures),
            View::Variables => Some(Self::Variables),
            View::Scope => Some(Self::Scope),
            View::Tutorial => Some(Self::Tutorial),
            View::Help | View::Welcome => None,
        }
    }
    pub fn id(self) -> egui::ViewportId {
        egui::ViewportId::from_hash_of(("nexuslab-panel", self.key()))
    }
    fn size(self) -> [f32; 2] {
        match self {
            Self::Workspace => [360.0, 640.0],
            Self::Files => [420.0, 620.0],
            Self::Hardware => [390.0, 720.0],
            Self::Command => [960.0, 560.0],
            _ => [1050.0, 720.0],
        }
    }
}
impl NexusApp {
    pub(super) fn panel_docked(&self, panel: Panel) -> bool {
        !self.closed_panels.contains(&panel) && !self.detached.contains(&panel)
    }
    pub(super) fn workbench_visible(&self) -> bool {
        self.view != View::Welcome
            || self.panel_docked(Panel::Command)
            || self.panel_docked(Panel::Workspace)
            || self.panel_docked(Panel::Files)
            || (self.hardware_visible && self.panel_docked(Panel::Hardware))
            || !self.detached.is_empty()
    }
    pub(super) fn open_panel(&mut self, panel: Panel) {
        self.closed_panels.remove(&panel);
        match panel {
            Panel::Editor => self.set_view(View::Workbench),
            Panel::Figures => self.set_view(View::Figures),
            Panel::Variables => self.set_view(View::Variables),
            Panel::Command => self.set_view(View::Command),
            Panel::Scope => self.set_view(View::Scope),
            Panel::Tutorial => self.set_view(View::Tutorial),
            Panel::Hardware => self.hardware_visible = true,
            Panel::Files => self.hardware_visible = false,
            Panel::Workspace => {}
        }
        if self.detached.contains(&panel) {
            self.focus_panel = Some(panel);
        }
    }
    pub(super) fn close_panel(&mut self, ctx: &egui::Context, panel: Panel) {
        // A pending import/plot may still finish and update the shared Session,
        // but it must respect the user's decision to dismiss the UI.
        if self.busy {
            self.reveal_job_result = false;
        }
        self.closed_panels.insert(panel);
        if self.detached.remove(&panel) {
            ctx.send_viewport_cmd_to(panel.id(), egui::ViewportCommand::Close);
        }
        self.themed_viewports.remove(&panel);
        if Panel::for_view(self.view) == Some(panel) {
            self.view = View::Welcome;
        }
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
    pub(super) fn close_all_panels(&mut self, ctx: &egui::Context) {
        for panel in Panel::all() {
            self.close_panel(ctx, panel);
        }
        self.view = View::Welcome;
        self.hardware_visible = false;
        self.toolkit.open = None;
        self.about_open = false;
    }
    pub(super) fn restore_panels(&mut self, ctx: &egui::Context) {
        for panel in self.detached.clone() {
            self.dock_panel(ctx, panel);
        }
        self.closed_panels.clear();
        self.view = View::Workbench;
        self.hardware_visible = false;
    }
    pub(super) fn detach_panel(&mut self, panel: Panel) {
        self.closed_panels.remove(&panel);
        self.themed_viewports.remove(&panel);
        self.detached.insert(panel);
        self.focus_panel = Some(panel);
        if panel == Panel::Scope && self.acquisition.status() == "Disconnected" {
            self.acquisition.mock(self.waveform, self.frequency);
        }
    }
    pub(super) fn dock_panel(&mut self, ctx: &egui::Context, panel: Panel) {
        self.closed_panels.remove(&panel);
        self.detached.remove(&panel);
        ctx.send_viewport_cmd_to(panel.id(), egui::ViewportCommand::Close);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
        if Panel::for_view(self.view) != Some(panel)
            && !matches!(panel, Panel::Files | Panel::Workspace)
        {
            self.open_panel(panel);
        }
    }
    pub(super) fn panel_button(&mut self, ui: &mut egui::Ui, panel: Panel) {
        let detached = self.detached.contains(&panel);
        if crate::icons::button(
            ui,
            if detached {
                crate::icons::Icon::Dock
            } else {
                crate::icons::Icon::PopOut
            },
            if detached {
                "Return this panel to the main window"
            } else {
                "Open this panel in a separate native window"
            },
            true,
        )
        .clicked()
        {
            if detached {
                self.dock_panel(ui.ctx(), panel);
            } else {
                self.detach_panel(panel);
            }
        }
        if crate::icons::button(
            ui,
            crate::icons::Icon::Close,
            "Close panel (reopen from View)",
            true,
        )
        .clicked()
        {
            self.close_panel(ui.ctx(), panel);
        }
    }
    pub(super) fn detached_placeholder(&mut self, ui: &mut egui::Ui, panel: Panel) {
        ui.heading(panel.title());
        ui.add_space(12.0);
        theme::group(ui).show(ui,|ui| {
            ui.label("This panel is open in a separate window. Its data and execution session are shared with the workbench.");
            ui.horizontal(|ui| {
                if ui.button("Bring window to front").clicked() { self.focus_panel=Some(panel); }
                if crate::icons::button(ui,crate::icons::Icon::Dock,"Dock in main window",true).clicked() { self.dock_panel(ui.ctx(),panel); }
            });
        });
    }
    pub(super) fn detached_windows(&mut self, ctx: &egui::Context) {
        let panels: Vec<_> = self.detached.iter().copied().collect();
        for panel in panels {
            ctx.show_viewport_immediate(
                panel.id(),
                egui::ViewportBuilder::default()
                    .with_icon(crate::branding::window_icon())
                    .with_title(format!("NexusLab — {}", panel.title()))
                    .with_inner_size(panel.size())
                    .with_min_inner_size([320.0, 280.0]),
                |child, class| {
                    if self.themed_viewports.insert(panel) {
                        child.send_viewport_cmd(egui::ViewportCommand::SetTheme(
                            if self.applied_dark {
                                egui::SystemTheme::Dark
                            } else {
                                egui::SystemTheme::Light
                            },
                        ));
                    }
                    if child.input(|i| i.viewport().close_requested()) {
                        self.close_panel(child, panel);
                        return;
                    }
                    self.shortcuts(child);
                    if class == egui::ViewportClass::Embedded {
                        let mut open = true;
                        egui::Window::new(panel.title())
                            .id(egui::Id::new(panel.key()))
                            .open(&mut open)
                            .default_size(panel.size())
                            .show(child, |ui| self.panel_contents(ui, panel));
                        if !open {
                            self.close_panel(child, panel);
                        }
                    } else {
                        egui::CentralPanel::default()
                            .frame(
                                egui::Frame::new()
                                    .fill(theme::palette(child).background)
                                    .inner_margin(8),
                            )
                            .show(child, |ui| {
                                ui.push_id(panel.key(), |ui| self.panel_contents(ui, panel))
                            });
                    }
                    if panel == Panel::Scope && !self.paused {
                        child.request_repaint();
                    } else {
                        child.request_repaint_after(Duration::from_millis(100));
                    }
                },
            );
        }
        if let Some(panel) = self.focus_panel.take() {
            ctx.send_viewport_cmd_to(panel.id(), egui::ViewportCommand::Focus);
        }
    }
    fn panel_contents(&mut self, ui: &mut egui::Ui, panel: Panel) {
        if self.about_open {
            ui.disable();
        }
        match panel {
            Panel::Editor => self.editor_ui(ui),
            Panel::Command => self.command_ui(ui),
            Panel::Figures => self.figures_ui(ui),
            Panel::Variables => self.variables_ui(ui),
            Panel::Scope => self.scope_ui(ui),
            Panel::Workspace => self.inspector_ui(ui),
            Panel::Files => self.browser_ui(ui),
            Panel::Hardware => self.hardware_ui(ui),
            Panel::Tutorial => self.academy_ui(ui),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn viewport_ids_and_saved_layout_keys_are_distinct() {
        let panels = Panel::all();
        let ids: std::collections::HashSet<_> = panels.iter().map(|p| p.id()).collect();
        assert_eq!(ids.len(), panels.len());
        for panel in panels {
            assert_eq!(Panel::from_key(panel.key()), Some(panel));
        }
        assert_eq!(Panel::for_view(View::Command), Some(Panel::Command));
    }
}
