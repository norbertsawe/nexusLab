use super::*;
impl NexusApp {
    pub(super) fn about_ui(&mut self, ctx: &egui::Context) {
        if !self.about_open {
            return;
        }
        if self.diagnostics_at.elapsed() > Duration::from_secs(1) {
            self.diagnostics = crate::diagnostics::read();
            self.diagnostics_at = Instant::now();
        }
        egui::Area::new(egui::Id::new("about_backdrop"))
            .order(egui::Order::Middle)
            .fixed_pos(ctx.screen_rect().min)
            .show(ctx, |ui| {
                let rect = ctx.screen_rect();
                ui.allocate_rect(rect, egui::Sense::click_and_drag());
                ui.painter()
                    .rect_filled(rect, 0, egui::Color32::from_black_alpha(50));
            });
        let mut open = true;
        let window_response=egui::Window::new("About NexusLab").id(egui::Id::new("about_window")).order(egui::Order::Foreground).open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,egui::Vec2::ZERO).default_width(510.0).show(ctx,|ui| {
            ui.horizontal(|ui| { ui.selectable_value(&mut self.about_diagnostics,false,"About");ui.selectable_value(&mut self.about_diagnostics,true,"System diagnostics"); });
            ui.separator();
            if self.about_diagnostics {
                egui::Grid::new("diagnostics_table").striped(true).show(ui,|ui| {
                    ui.label("Graphics");ui.label(&self.graphics);ui.end_row();
                    ui.label("Process resident memory");ui.label(self.diagnostics.resident_bytes.map(|n|format!("{:.1} MiB",n as f64/1048576.0)).unwrap_or_else(||"Unavailable".into()));ui.end_row();
                    ui.label("Workspace payload");ui.label(format!("{:.1} KiB",self.variables.values().map(Value::bytes).sum::<usize>() as f64/1024.0));ui.end_row();
                    ui.label("Active OS threads");ui.label(self.diagnostics.threads.map(|n|n.to_string()).unwrap_or_else(||"Unavailable".into()));ui.end_row();
                    ui.label("Acquisition");ui.label(self.acquisition.status());ui.end_row();
                    ui.label("Script worker");ui.label(if self.busy { "Busy" } else { "Idle" });ui.end_row();
                });
                ui.label("Live process measurements refresh once per second. Workspace bytes count numeric/text payloads; process memory includes renderer, workers, and allocations.");
            } else {
                ui.heading("NexusLab - Open-Source Hardware & Numerical Workbench");
                ui.label(RichText::new(format!("v{} Pro",env!("CARGO_PKG_VERSION"))).strong());
                ui.add_space(8.0);
                ui.label("Author / Lead Architect: Norbert John");
                ui.add_space(8.0);
                ui.label("A high-performance, native Rust numerical engine and signal processing platform built for students, researchers, and hardware engineers.");
                ui.label("Open-source · MIT License");
            }
            ui.separator();
            if ui.button("Close").clicked() { self.about_open=false; }
        });
        if let Some(response) = window_response {
            ctx.memory_mut(|m| m.set_modal_layer(response.response.layer_id));
        }
        if !open || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.about_open = false;
        }
    }
}
