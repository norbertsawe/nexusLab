use super::*;
#[derive(Default)]
pub(super) struct Completion {
    pub open: bool,
    pub candidates: Vec<String>,
    pub index: usize,
    pub prefix_start: usize,
    pub draft: String,
    pub cursor_to_end: bool,
    pub prefix_end: usize,
    pub cursor_byte: usize,
}
const BUILTINS: &[&str] = &[
    "sin",
    "cos",
    "tan",
    "sqrt",
    "exp",
    "log",
    "log10",
    "abs",
    "floor",
    "ceil",
    "round",
    "fft",
    "ifft",
    "filter",
    "fir",
    "sosfilt",
    "freqz",
    "psd",
    "butter_lowpass",
    "butter_highpass",
    "butter_bandpass",
    "hann",
    "hamming",
    "blackmanharris",
    "fir_lowpass",
    "fir_highpass",
    "fir_bandpass",
    "iir_lowpass",
    "iir_highpass",
    "iir_bandpass",
    "stats",
    "mean",
    "median",
    "std",
    "rms",
    "ptp",
    "sum",
    "min",
    "max",
    "norm",
    "diff",
    "cumsum",
    "sort",
    "inv",
    "det",
    "solve",
    "size",
    "length",
    "zeros",
    "ones",
    "eye",
    "diag",
    "reshape",
    "transpose",
    "linspace",
    "randn",
    "polyfit",
    "polyval",
    "plot",
    "scatter",
    "bar",
    "figure",
    "title",
    "xlabel",
    "ylabel",
    "grid",
    "hold",
    "row",
    "column",
    "disp",
    "help",
    "who",
    "whos",
    "clear",
    "clc",
    "pwd",
    "cd",
    "run",
    "serial_ports",
    "usb_devices",
];
impl NexusApp {
    pub(super) fn completion_candidates(&mut self) {
        let end = self.completion.cursor_byte.min(self.input.len());
        let start = self.input[..end]
            .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .map_or(0, |i| i + 1);
        let prefix = &self.input[start..end];
        let mut candidates: Vec<String> = self
            .variables
            .keys()
            .map(String::as_str)
            .chain(BUILTINS.iter().copied())
            .filter(|s| s.starts_with(prefix))
            .map(str::to_string)
            .collect();
        candidates.sort();
        candidates.dedup();
        self.completion = Completion {
            open: !candidates.is_empty(),
            candidates,
            index: 0,
            prefix_start: start,
            draft: self.input.clone(),
            cursor_to_end: false,
            prefix_end: end,
            cursor_byte: end,
        };
    }
    pub(super) fn accept_completion(&mut self) {
        if let Some(value) = self.completion.candidates.get(self.completion.index) {
            self.input.replace_range(
                self.completion.prefix_start..self.completion.prefix_end,
                value,
            );
            self.completion.cursor_byte = self.completion.prefix_start + value.len();
        }
        self.completion.open = false;
        self.focus_command = true;
        self.completion.cursor_to_end = true;
    }
    pub(super) fn completion_popup(&mut self, ui: &mut egui::Ui, anchor: egui::Rect) {
        if !self.completion.open {
            return;
        }
        if self.input != self.completion.draft {
            self.completion_candidates();
        }
        let screen = ui.ctx().screen_rect();
        let height = (self.completion.candidates.len().min(8) as f32 * 26.0 + 10.0).min(220.0);
        let position = egui::pos2(
            anchor.left(),
            (anchor.top() - height - 4.0).max(screen.top() + 4.0),
        );
        let mut choose = None;
        egui::Area::new(egui::Id::new((
            "command_completion",
            ui.ctx().viewport_id(),
        )))
        .order(egui::Order::Foreground)
        .fixed_pos(position)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(240.0);
                egui::ScrollArea::vertical()
                    .max_height(208.0)
                    .show(ui, |ui| {
                        for (index, candidate) in self.completion.candidates.iter().enumerate() {
                            let response =
                                ui.selectable_label(index == self.completion.index, candidate);
                            if index == self.completion.index {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                            if response.clicked() {
                                choose = Some(index);
                            }
                        }
                    });
            });
        });
        if let Some(index) = choose {
            self.completion.index = index;
            self.accept_completion();
        }
    }
}
