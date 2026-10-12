use super::*;
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Tool {
    Filter,
    Spectral,
    Statistics,
}
pub(super) struct Toolkit {
    pub open: Option<Tool>,
    pub variable: String,
    pub rate: f64,
    pub segment: usize,
    pub iir: bool,
    pub kind: usize,
    pub order: usize,
    pub taps: usize,
    pub low: f64,
    pub high: f64,
    pub output: String,
    pub statistics: Option<nexus_core::Statistics>,
    pub error: Option<String>,
}
impl Default for Toolkit {
    fn default() -> Self {
        Self {
            open: None,
            variable: String::new(),
            rate: 1000.0,
            segment: 256,
            iir: true,
            kind: 0,
            order: 4,
            taps: 63,
            low: 100.0,
            high: 200.0,
            output: "filter_coefficients".into(),
            statistics: None,
            error: None,
        }
    }
}
impl NexusApp {
    pub(super) fn open_tool(&mut self, tool: Tool) {
        self.toolkit.open = Some(tool);
        self.toolkit.error = None;
        self.toolkit.statistics = None;
        self.toolkit.variable = self
            .selected
            .clone()
            .or_else(|| {
                self.variables
                    .iter()
                    .find(|(n, v)| n.as_str() != "pi" && !matches!(v, Value::Text(_)))
                    .map(|(n, _)| n.clone())
            })
            .unwrap_or_default();
        if tool == Tool::Spectral {
            let eligible = |v: &Value| matches!(v,Value::Real(m) if m.data.len()>=4);
            if !self
                .variables
                .get(&self.toolkit.variable)
                .is_some_and(eligible)
            {
                self.toolkit.variable = self
                    .variables
                    .iter()
                    .find(|(_, v)| eligible(v))
                    .map(|(n, _)| n.clone())
                    .unwrap_or_default();
            }
            if let Some(Value::Real(m)) = self.variables.get(&self.toolkit.variable) {
                self.toolkit.segment = self.toolkit.segment.min(m.data.len()).max(4);
            }
        }
        if tool == Tool::Statistics && !self.toolkit.variable.is_empty() {
            self.submit(Request::Statistics(self.toolkit.variable.clone()));
        }
    }
    pub(super) fn toolkit_ui(&mut self, ctx: &egui::Context) {
        let Some(tool) = self.toolkit.open else {
            return;
        };
        let mut open = true;
        let mut action = 0;
        let title = match tool {
            Tool::Filter => "Filter Design",
            Tool::Spectral => "Spectral Analyzer",
            Tool::Statistics => "Statistics Summary",
        };
        let previous_variable = self.toolkit.variable.clone();
        egui::Window::new(title).id(egui::Id::new("toolkit")).open(&mut open).default_width(460.0).show(ctx,|ui| { if self.busy {ui.disable();}
            if tool!=Tool::Filter {
                egui::ComboBox::from_id_salt("tool_variable").selected_text(if self.toolkit.variable.is_empty() { "Select a numeric variable" } else { &self.toolkit.variable }).show_ui(ui,|ui| {
                    for (name,value) in &self.variables { if !matches!(value,Value::Text(_)) { ui.selectable_value(&mut self.toolkit.variable,name.clone(),name); } }
                });
            }
            match tool {
                Tool::Statistics=> {
                    ui.label("All array elements; complex arrays use magnitudes. Standard deviation uses N−1.");
                    if ui.add_enabled(!self.busy && !self.toolkit.variable.is_empty(),egui::Button::new("Calculate summary")).clicked() { action=1; }
                    if let Some(s)=&self.toolkit.statistics {
                        egui::Grid::new("statistics_summary").striped(true).spacing(egui::vec2(12.0,4.0)).show(ui,|ui| { ui.label("Samples");ui.monospace(s.count.to_string());ui.end_row();
                            for (name,value) in [("Mean",s.mean),("Median",s.median),("Standard deviation",s.std_dev),("RMS",s.rms),("Peak-to-peak",s.peak_to_peak),("Minimum",s.minimum),("Maximum",s.maximum)] { ui.label(name);ui.monospace(format!("{value:.8}"));ui.end_row(); }
                        });
                    }
                }
                Tool::Spectral=> {
                    ui.label("One-sided Welch PSD: Hann window, 50% overlap, segment mean removal.");
                    ui.horizontal(|ui| { ui.label("Sample rate (Hz)");ui.add(egui::DragValue::new(&mut self.toolkit.rate).range(0.001..=1e9).speed(10.0)); });
                    ui.horizontal(|ui| { ui.label("Segment length");ui.add(egui::DragValue::new(&mut self.toolkit.segment).range(4..=65536)); });
                    if ui.add_enabled(!self.busy && self.variables.get(&self.toolkit.variable).is_some_and(|v|matches!(v,Value::Real(m) if m.data.len()>=4)),egui::Button::new("Compute PSD and FFT")).clicked() { action=2; }
                    ui.label("PSD columns: frequency (Hz), power/Hz. FFT magnitudes are stored separately. Real samples are required; use column(A,n) to select matrix channels.");
                }
                Tool::Filter=> {
                    ui.horizontal(|ui| { ui.selectable_value(&mut self.toolkit.iir,true,"IIR Butterworth");ui.selectable_value(&mut self.toolkit.iir,false,"FIR windowed-sinc"); });
                    egui::ComboBox::from_id_salt("filter_kind").selected_text(["Low-pass","High-pass","Band-pass"][self.toolkit.kind]).show_ui(ui,|ui| { for (i,label) in ["Low-pass","High-pass","Band-pass"].iter().enumerate() { ui.selectable_value(&mut self.toolkit.kind,i,*label); } });
                    egui::Grid::new("filter_parameters").show(ui,|ui| {
                        ui.label("Sample rate (Hz)");ui.add(egui::DragValue::new(&mut self.toolkit.rate).range(0.001..=1e9));ui.end_row();
                        ui.label("Cutoff / lower edge (Hz)");ui.add(egui::DragValue::new(&mut self.toolkit.low).range(0.0001..=1e9));ui.end_row();
                        if self.toolkit.kind==2 { ui.label("Upper edge (Hz)");ui.add(egui::DragValue::new(&mut self.toolkit.high).range(0.0001..=1e9));ui.end_row(); }
                        if self.toolkit.iir { ui.label("Prototype order");ui.add(egui::DragValue::new(&mut self.toolkit.order).range(1..=12)); } else { ui.label("Odd tap count");ui.add(egui::DragValue::new(&mut self.toolkit.taps).range(3..=4095)); } ui.end_row();
                        ui.label("Output variable");ui.add_sized([220.0,24.0],egui::TextEdit::singleline(&mut self.toolkit.output));ui.end_row();
                    });
                    ui.label("IIR exports SOS rows [b0 b1 b2 a0 a1 a2]; band-pass digital order is twice the prototype order. FIR exports taps.");
                    if ui.add_enabled(!self.busy,egui::Button::new("Generate coefficients and response")).clicked() { action=3; }
                    ui.label("Apply using sosfilt(samples, coefficients) or filter(samples, taps).");
                }
            }
            if let Some(error)=&self.toolkit.error { ui.colored_label(theme::palette(ui.ctx()).error,error); }
        });
        if previous_variable != self.toolkit.variable {
            self.toolkit.statistics = None;
            self.toolkit.error = None;
            if tool == Tool::Spectral {
                if let Some(Value::Real(m)) = self.variables.get(&self.toolkit.variable) {
                    self.toolkit.segment = self.toolkit.segment.min(m.data.len()).max(4);
                }
            }
        }
        if !open || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.toolkit.open = None;
        }
        if action == 1 {
            self.toolkit.statistics = None;
            self.submit(Request::Statistics(self.toolkit.variable.clone()));
        }
        if action == 2 {
            let name = &self.toolkit.variable;
            let fs = self.toolkit.rate;
            let n = self.toolkit.segment;
            let id = self.next_figure();
            let code=format!("{name}_psd = psd({name},{fs},{n});\n{name}_fft = abs(fft({name}));\nfigure({id}); plot(column({name}_psd,1),column({name}_psd,2)); title('Welch PSD: {name}'); xlabel('Frequency (Hz)'); ylabel('Power / Hz');\nfigure({}); plot({name}_fft);title('FFT magnitude: {name}');xlabel('FFT bin');ylabel('Magnitude');",(id+1).min(16));
            self.run_code(code, "Spectral Analyzer".into(), false);
        }
        if action == 3 {
            let t = &self.toolkit;
            if !nexus_script::workspace::valid_name(&t.output) || t.output == "pi" {
                self.toolkit.error = Some("Choose a valid output identifier other than pi.".into());
                return;
            }
            if !t.iir && t.taps.is_multiple_of(2) {
                self.toolkit.error = Some("FIR requires an odd tap count.".into());
                return;
            }
            let function = format!(
                "{}_{}",
                if t.iir { "butter" } else { "fir" },
                ["lowpass", "highpass", "bandpass"][t.kind]
            );
            let size = if t.iir { t.order } else { t.taps };
            let args = if t.kind == 2 {
                format!("{size},{},{},{}", t.low, t.high, t.rate)
            } else {
                format!("{size},{},{}", t.low, t.rate)
            };
            let name = t.output.clone();
            let rate = t.rate;
            let id = self.next_figure();
            self.toolkit.error = None;
            self.run_code(format!("{name}={function}({args});\n{name}_response=freqz({name},{rate});\nfigure({id});plot(column({name}_response,1),column({name}_response,2));title('Filter frequency response');xlabel('Frequency (Hz)');ylabel('Gain (dB)');"),"Filter Design".into(),false);
        }
    }
    fn next_figure(&self) -> u32 {
        (self.figures.keys().max().copied().unwrap_or(0) + 1).min(15)
    }
}
