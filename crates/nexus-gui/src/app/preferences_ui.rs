use super::*;
use crate::preferences::Theme;

impl NexusApp {
    pub(super) fn apply_preferences(&mut self, ctx: &egui::Context) {
        self.preferences.normalize();
        self.applied_dark = self.preferences.dark(ctx);
        theme::apply(ctx, &self.preferences);
        self.themed_viewports.clear();
        for panel in &self.detached {
            ctx.request_repaint_of(panel.id());
        }
        ctx.request_repaint();
    }
    pub(super) fn preferences_ui(&mut self, ctx: &egui::Context) {
        if !self.preferences_open {
            return;
        }
        let previous = self.preferences.clone();
        let mut open = true;
        let mut restore = false;
        egui::Window::new("Preferences")
            .id(egui::Id::new("preferences"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(430.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.heading("Appearance");
                ui.horizontal(|ui| {
                    ui.label("Color theme");
                    for (value, label) in [(Theme::Light, "Light"), (Theme::Dark, "Dark"), (Theme::System, "System")] {
                        ui.selectable_value(&mut self.preferences.theme, value, label);
                    }
                });
                egui::Grid::new("font_preferences").spacing(egui::vec2(12.0, 6.0)).show(ui, |ui| {
                    ui.label("Interface font size");
                    ui.add(egui::Slider::new(&mut self.preferences.ui_font_size, 10.0..=18.0).step_by(0.5).suffix(" px")); ui.end_row();
                    ui.label("Editor and command font size");
                    ui.add(egui::Slider::new(&mut self.preferences.code_font_size, 10.0..=24.0).step_by(0.5).suffix(" px")); ui.end_row();
                });
                ui.checkbox(&mut self.preferences.striped_rows, "Alternate explorer and table row backgrounds");
                ui.checkbox(&mut self.preferences.line_numbers, "Show editor line numbers");
                ui.add_space(8.0);
                ui.heading("Workbench");
                ui.checkbox(&mut self.preferences.show_toolbar, "Show action toolbar");
                ui.checkbox(&mut self.preferences.show_status_bar, "Show status bar");
                ui.checkbox(&mut self.preferences.restore_layout, "Restore panel layout on startup");
                ui.label(RichText::new("Closing a panel preserves its documents, variables, and figures. Reopen panels from View.").small().color(theme::palette(ui.ctx()).muted));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Restore panels").clicked() { restore = true; }
                    if ui.button("Reset preferences").clicked() { self.preferences = crate::preferences::Preferences::default(); }
                    if ui.button("Close").clicked() { self.preferences_open = false; }
                });
            });
        if restore {
            self.restore_panels(ctx);
        }
        if self.preferences != previous {
            self.apply_preferences(ctx);
            match self.preferences.save() {
                Ok(()) => self.status = "Preferences saved".into(),
                Err(e) => self.notify(format!("Cannot save preferences: {e}"), false),
            }
        }
        if !open || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.preferences_open = false;
        }
    }
}
