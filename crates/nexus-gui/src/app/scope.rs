use super::*;
impl NexusApp {
    pub(super) fn hardware(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("hardware")
            .exact_width(239.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::palette(ctx).background)
                    .inner_margin(8),
            )
            .show(ctx, |ui| self.hardware_ui(ui));
    }
    pub(super) fn hardware_ui(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.horizontal(|ui| {
                theme::label(ui, "Hardware Manager");
                self.panel_button(ui, Panel::Hardware);
            });
            ui.add_space(10.0);
            theme::group(ui).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Acquisition").strong());
                });
                ui.add_space(6.0);
                theme::label(ui, "Serial port");
                egui::ComboBox::from_id_salt("port")
                    .selected_text(&self.config.port)
                    .show_ui(ui, |ui| {
                        for p in &self.ports {
                            ui.selectable_value(&mut self.config.port, p.name.clone(), &p.name)
                                .on_hover_text(&p.description);
                        }
                        if cfg!(windows) {
                            for p in nexus_hardware::windows_com_names() {
                                if !self.ports.iter().any(|v| v.name == p) {
                                    ui.selectable_value(&mut self.config.port, p.clone(), p);
                                }
                            }
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.port)
                        .desired_width(150.0)
                        .hint_text("COM3 or /dev/ttyUSB0"),
                );
                theme::label(ui, "Baud rate");
                ui.add(
                    egui::DragValue::new(&mut self.config.baud)
                        .range(1..=4_000_000)
                        .speed(100),
                );
                egui::CollapsingHeader::new("Advanced settings").show(ui, |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.config.sample_rate)
                            .range(1.0..=10_000_000.0)
                            .speed(100)
                            .suffix(" Sa/s"),
                    );
                    egui::ComboBox::from_id_salt("framing")
                        .selected_text(format!("{:?}", self.config.framing))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.config.framing,
                                Framing::Lines,
                                "Text lines",
                            );
                            ui.selectable_value(
                                &mut self.config.framing,
                                Framing::Float32Le,
                                "Float32 LE",
                            );
                        });
                    egui::ComboBox::from_id_salt("parity")
                        .selected_text(format!("Parity: {:?}", self.config.parity))
                        .show_ui(ui, |ui| {
                            for p in [
                                serialport_reexport::Parity::None,
                                serialport_reexport::Parity::Odd,
                                serialport_reexport::Parity::Even,
                            ] {
                                ui.selectable_value(&mut self.config.parity, p, format!("{p:?}"));
                            }
                        });
                    egui::ComboBox::from_id_salt("flow")
                        .selected_text(format!("Flow: {:?}", self.config.flow_control))
                        .show_ui(ui, |ui| {
                            for f in [
                                serialport_reexport::FlowControl::None,
                                serialport_reexport::FlowControl::Software,
                                serialport_reexport::FlowControl::Hardware,
                            ] {
                                ui.selectable_value(
                                    &mut self.config.flow_control,
                                    f,
                                    format!("{f:?}"),
                                );
                            }
                        });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Connect").clicked() {
                        self.acquisition.connect(self.config.clone());
                        self.samples.clear();
                        self.selected = None;
                    }
                    if ui.small_button("Stop").clicked() {
                        self.acquisition.disconnect();
                    }
                });
                if ui.small_button("Refresh devices").clicked() {
                    match nexus_hardware::ports() {
                        Ok(p) => {
                            self.ports = p;
                            self.port_error = None;
                        }
                        Err(e) => self.port_error = Some(e.to_string()),
                    }
                }
                if let Some(e) = &self.port_error {
                    ui.label(
                        RichText::new(e)
                            .small()
                            .color(theme::palette(ui.ctx()).muted),
                    );
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new(self.acquisition.status())
                        .small()
                        .color(theme::palette(ui.ctx()).muted),
                );
            });
            ui.add_space(14.0);
            theme::group(ui).show(ui, |ui| {
                theme::label(ui, "Mock Generator");
                ui.add_space(4.0);
                let mut changed = false;
                egui::ComboBox::from_id_salt("wave")
                    .selected_text(format!("{:?}", self.waveform))
                    .show_ui(ui, |ui| {
                        for w in [
                            Waveform::Sine,
                            Waveform::Square,
                            Waveform::Triangle,
                            Waveform::Saw,
                            Waveform::Noise,
                        ] {
                            changed |= ui
                                .selectable_value(&mut self.waveform, w, format!("{w:?}"))
                                .changed();
                        }
                    });
                changed |= ui
                    .add(
                        egui::Slider::new(&mut self.frequency, 10.0..=20000.0)
                            .logarithmic(true)
                            .suffix(" Hz"),
                    )
                    .changed();
                ui.label(
                    RichText::new("128 kSa/s  ·  5 V peak-to-peak")
                        .small()
                        .color(theme::palette(ui.ctx()).muted),
                );
                if ui.button("Use mock signal").clicked() || changed {
                    self.acquisition.mock(self.waveform, self.frequency);
                    self.selected = None;
                    self.samples.clear();
                }
            });
        });
    }
    fn scope_points(&self) -> Vec<[f64; 2]> {
        let n = 1280.min(self.samples.len());
        let mut start = self.samples.len() - n;
        if self.trigger && self.samples.len() > n * 2 {
            let begin = self.samples.len() - n * 2;
            for i in (begin + 1..self.samples.len() - n).rev() {
                if self.samples[i - 1][1] < self.threshold && self.samples[i][1] >= self.threshold {
                    start = i;
                    break;
                }
            }
        }
        let offset = self.samples.get(start).map(|p| p[0]).unwrap_or(0.0);
        self.samples
            .iter()
            .skip(start)
            .take(n)
            .map(|p| [(p[0] - offset) * 1000.0, p[1]])
            .collect()
    }
    pub(super) fn scope_canvas(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::palette(ctx).background)
                    .inner_margin(egui::Margin::symmetric(8, 8)),
            )
            .show(ctx, |ui| self.scope_ui(ui));
    }
    pub(super) fn scope_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Live Scope");
            self.panel_button(ui, Panel::Scope);
            if crate::icons::button(
                ui,
                if self.paused {
                    crate::icons::Icon::Run
                } else {
                    crate::icons::Icon::Pause
                },
                if self.paused {
                    "Resume acquisition"
                } else {
                    "Pause acquisition"
                },
                true,
            )
            .clicked()
            {
                self.paused = !self.paused;
            }
            if crate::icons::button(ui, crate::icons::Icon::Fit, "Auto-scale scope", true).clicked()
            {
                self.scope_auto_scale = true;
            }
        });
        ui.add_space(6.0);
        let points = self.scope_points();
        let max = points
            .iter()
            .map(|p| p[1])
            .fold(f64::NEG_INFINITY, f64::max);
        let min = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let rms =
            (points.iter().map(|p| p[1] * p[1]).sum::<f64>() / points.len().max(1) as f64).sqrt();
        let plot_height = ((ui.available_height() - 140.0) * 0.59).max(110.0);
        theme::group(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Channel 1 · voltage").strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.checkbox(&mut self.trigger, "Trigger");
                    if self.trigger {
                        ui.add(
                            egui::DragValue::new(&mut self.threshold)
                                .speed(0.05)
                                .suffix(" V"),
                        );
                    }
                });
            });
            let plot = Plot::new("scope")
                .height(plot_height)
                .show_background(false)
                .show_grid([true, true])
                .legend(Legend::default())
                .x_axis_label("Time (ms)")
                .y_axis_label("Amplitude")
                .allow_zoom(true)
                .allow_drag(true);
            plot.show(ui, |p| {
                if self.scope_auto_scale {
                    p.set_auto_bounds(egui::Vec2b::TRUE);
                }
                p.line(
                    Line::new("Ch 1", PlotPoints::from(points))
                        .color(GREEN)
                        .width(1.6_f32),
                );
                if self.trigger {
                    p.hline(HLine::new("Trigger", self.threshold).color(BLUE.gamma_multiply(0.65)));
                }
            });
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{:.0} Hz", self.frequency))
                        .small()
                        .color(theme::palette(ui.ctx()).muted),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("{:.3} V RMS", rms))
                        .small()
                        .color(theme::palette(ui.ctx()).muted),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "{:.3} V p-p",
                        if max.is_finite() { max - min } else { 0.0 }
                    ))
                    .small()
                    .color(theme::palette(ui.ctx()).muted),
                );
            });
        });
        ui.add_space(12.0);
        let spectrum_height = (ui.available_height() - 66.0).max(70.0);
        theme::group(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Frequency spectrum").strong());
                ui.label(
                    RichText::new("Live · Hann · 4096-point FFT")
                        .small()
                        .color(theme::palette(ui.ctx()).muted),
                );
            });
            Plot::new("spectrum")
                .height(spectrum_height)
                .show_background(false)
                .include_x(0.0)
                .include_x(64.0)
                .include_y(-100.0)
                .include_y(20.0)
                .x_axis_label("Frequency (kHz)")
                .y_axis_label("dBV")
                .show_grid([true, true])
                .show(ui, |p| {
                    p.line(
                        Line::new("FFT magnitude", PlotPoints::from(self.spectrum.clone()))
                            .color(BLUE)
                            .width(1.5_f32),
                    );
                });
        });
        self.scope_auto_scale = false;
    }
}
use nexus_hardware::serialport_export as serialport_reexport;
