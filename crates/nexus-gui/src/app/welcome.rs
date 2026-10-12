use super::*;

impl NexusApp {
    pub(super) fn welcome_ui(&mut self, ui: &mut egui::Ui) {
        let colors = theme::palette(ui.ctx());
        let size = ui.available_size();
        // A quiet NexusLab waveform mark, drawn as vectors so it remains sharp
        // at every display scale. The reference informs layout, not branding.
        let mark_size = size.x.min(size.y).min(280.0);
        ui.add_space(((size.y - mark_size - 150.0) / 2.0).max(24.0));
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(mark_size, mark_size * 0.65),
                egui::Sense::hover(),
            );
            let color = if ui.visuals().dark_mode {
                egui::Color32::from_rgb(42, 42, 42)
            } else {
                egui::Color32::from_rgb(219, 223, 228)
            };
            let points = [
                (0.03, 0.5),
                (0.2, 0.5),
                (0.3, 0.25),
                (0.43, 0.8),
                (0.58, 0.08),
                (0.72, 0.65),
                (0.84, 0.5),
                (0.97, 0.5),
            ]
            .into_iter()
            .map(|(x, y)| {
                egui::pos2(
                    rect.left() + x * rect.width(),
                    rect.top() + y * rect.height(),
                )
            })
            .collect();
            ui.painter().add(egui::Shape::line(
                points,
                egui::Stroke::new(12.0_f32, color),
            ));
            ui.add_space(22.0);
            ui.label(RichText::new("NexusLab").size(20.0).color(colors.muted));
            ui.add_space(18.0);
            let width = size.x.min(380.0);
            ui.allocate_ui_with_layout(
                egui::vec2(width, 130.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    for (label, keys, action) in [
                        ("New script", "Ctrl+N", 0),
                        ("Open file", "Ctrl+O", 1),
                        ("Command window", "F6", 2),
                        ("Restore panels", "Ctrl+Shift+P", 3),
                        ("Preferences", "Ctrl+,", 4),
                    ] {
                        ui.horizontal(|ui| {
                            let response = ui.add_sized(
                                [width - 135.0, 22.0],
                                egui::Button::new(RichText::new(label).color(colors.muted))
                                    .frame(false),
                            );
                            ui.label(RichText::new(keys).monospace().small().color(colors.muted));
                            if response.clicked() {
                                match action {
                                    0 => self.new_document("untitled.nx", "", false),
                                    1 => self.open_dialog(),
                                    2 => self.open_panel(Panel::Command),
                                    3 => self.restore_panels(ui.ctx()),
                                    _ => self.preferences_open = true,
                                }
                            }
                        });
                    }
                },
            );
        });
    }
}
