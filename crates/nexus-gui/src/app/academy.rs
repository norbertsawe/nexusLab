use super::*;

struct Example {
    title: &'static str,
    code: &'static str,
    result: &'static str,
}
struct Lesson {
    group: &'static str,
    title: &'static str,
    introduction: &'static str,
    notes: &'static [&'static str],
    examples: &'static [Example],
}
const LESSONS: &[Lesson] = &[
    Lesson {
        group: "1. Getting Started", title: "The numerical workspace",
        introduction: "The command window evaluates expressions and stores named results in the workspace. Use a semicolon to suppress output, and % to write a comment. All lessons run in your current session.",
        notes: &["Open the command window with F6. Use Up and Down to revisit commands; Tab opens completion.", "Inspect a variable in the Workspace panel or open the Variable Editor. Save a .nxw workspace to keep your numerical data."],
        examples: &[Example { title: "Your first calculation", code: "% Store a result and inspect it\nacademy_radius = 3;\nacademy_area = pi * academy_radius.^2;\nacademy_area", result: "The area is approximately 28.27433388. Lesson variables use the academy_ prefix." }],
    },
    Lesson {
        group: "2. Vectors & Matrices", title: "Ranges and element-wise math",
        introduction: "start:step:stop creates an evenly spaced row vector. With two values, start:stop uses a step of one. Use .* , ./ and .^ to calculate independently at each element; * and ^ perform matrix operations.",
        notes: &["A single quote transposes a real array and conjugate-transposes a complex vector.", "An empty range is valid. Indices begin at 1, and linear indexing walks down columns."],
        examples: &[Example { title: "Generate samples", code: "academy_t = 0:0.01:1;\nacademy_wave = sin(2*pi*3*academy_t);\nacademy_energy = academy_wave .* academy_wave;\nsize(academy_t)", result: "101 samples in a 1 × 101 row vector." }, Example { title: "Rows, columns, and element-wise operators", code: "academy_v = [1 2 3];\nacademy_column = academy_v';\nacademy_squared = academy_v.^2;\nacademy_ratio = academy_squared ./ academy_v;\nacademy_column", result: "A 3 × 1 column vector. The squared values are [1 4 9] and the ratios are [1 2 3]." }],
    },
    Lesson {
        group: "2. Vectors & Matrices", title: "Slicing and indexed assignment",
        introduction: "Use a colon to select every row or column. Ranges and index vectors select subsets; end means the last position in the dimension being indexed. A(:) collects the full array into a column vector.",
        notes: &["A(row_indices, column_indices) selects every combination of the requested rows and columns.", "Indexed assignment accepts matching data or a scalar. It changes existing elements; automatic array growth and logical masks are not supported."],
        examples: &[Example { title: "Select part of an array", code: "academy_A = [1 2 3; 4 5 6; 7 8 9];\nacademy_second_column = academy_A(:,2);\nacademy_corner = academy_A(2:end,1:2);\nacademy_flat = academy_A(:);\nacademy_corner", result: "The corner is [4 5; 7 8]. The flattened array is [1; 4; 7; 2; 5; 8; 3; 6; 9]." }, Example { title: "Replace selected values", code: "academy_edit = [1 2 3; 4 5 6];\nacademy_edit(:,2) = [20;50];\nacademy_edit(end) = 60;\nacademy_edit", result: "The edited array is [1 20 3; 4 50 60]. Invalid indices leave the array unchanged." }],
    },
    Lesson {
        group: "2. Vectors & Matrices", title: "Linear systems and matrix powers",
        introduction: "A * B performs matrix multiplication. A \\ b solves a linear system; rectangular systems use the pseudoinverse for a least-squares or minimum-norm solution. A / B solves the corresponding right-division problem.",
        notes: &["Use a solve rather than explicitly forming an inverse when finding a solution.", "det and inv require square matrices. Matrix powers support integer exponents; negative powers use the inverse. Decompositions are limited to 256 rows and columns."],
        examples: &[Example { title: "Solve and check the residual", code: "academy_system = [3 1;1 2];\nacademy_b = [9;8];\nacademy_solution = academy_system \\ academy_b;\nacademy_residual = norm(academy_system * academy_solution - academy_b);\nacademy_solution", result: "The solution is [2;3], with a residual close to zero." }, Example { title: "Determinant, inverse, and power", code: "academy_M = [1 2;0 1];\nacademy_determinant = det(academy_M);\nacademy_inverse = inv(academy_M);\nacademy_cube = academy_M^3;\nacademy_identity = academy_M * academy_inverse;\nacademy_cube", result: "The determinant is 1 and the cube is [1 6;0 1]." }],
    },
    Lesson {
        group: "2. Vectors & Matrices", title: "Eigenvalues and pseudoinverses",
        introduction: "eig(A) returns eigenvalues as a column vector. A real matrix can have complex eigenvalues. pinv(A) computes the Moore–Penrose pseudoinverse, including rectangular and rank-deficient arrays.",
        notes: &["Eigenvalues are not sorted. This release provides the single-output form of eig, without eigenvectors or generalized eigenproblems.", "pinv(A,tolerance) optionally sets an absolute singular-value cutoff. The default is max(rows,columns) × machine epsilon × largest singular value."],
        examples: &[Example { title: "Complex eigenvalues", code: "academy_rotation = [0 -1;1 0];\nacademy_eigenvalues = eig(academy_rotation);\nacademy_eigenvalues", result: "A 2 × 1 complex vector containing +i and −i, in either order." }, Example { title: "A rank-deficient array", code: "academy_rankdef = [1 2;2 4];\nacademy_pseudoinverse = pinv(academy_rankdef);\nacademy_pinv_error = norm(academy_rankdef * academy_pseudoinverse * academy_rankdef - academy_rankdef);\nacademy_pseudoinverse", result: "The pseudoinverse is [0.04 0.08;0.08 0.16]. The identity error is close to zero." }],
    },
    Lesson {
        group: "2. Vectors & Matrices", title: "Polynomial fitting and statistics",
        introduction: "polyfit(x,y,degree) fits a polynomial by least squares. Coefficients are returned in descending powers. polyval(coefficients,x) evaluates the fitted curve at new positions.",
        notes: &["Use stats(array) for mean, median, sample standard deviation, RMS, and peak-to-peak range.", "Polynomial degree is limited to 16. For reliable high-degree fits, center and scale the sample coordinates first."],
        examples: &[Example { title: "Fit a quadratic", code: "academy_fit_x = 0:4;\nacademy_fit_y = [1 4 9 16 25];\nacademy_coefficients = polyfit(academy_fit_x,academy_fit_y,2);\nacademy_predictions = polyval(academy_coefficients,academy_fit_x);\nacademy_coefficients", result: "The coefficients are approximately [1 2 1], describing x² + 2x + 1." }, Example { title: "Summarize measurements", code: "academy_measurements = [1 2 3 4 5];\nstats(academy_measurements)", result: "Mean and median are 3. Peak-to-peak is 4; RMS is approximately 3.31662479." }],
    },
    Lesson {
        group: "3. Plotting Data", title: "Figures and axes",
        introduction: "plot(x,y) creates a figure from matching arrays. Give your analysis a title and axis labels. Figures keep their data when you return to the editor or close a panel.",
        notes: &["Hover to read coordinates. Drag with the middle mouse button to pan; the wheel zooms. Ctrl restricts zoom to X, and Shift restricts it to Y.", "Use the figure toolbar to export SVG, high-resolution PNG, or complete raw CSV data."],
        examples: &[Example { title: "Plot an oscillation", code: "academy_plot_t = 0:0.005:1;\nacademy_plot_y = sin(2*pi*5*academy_plot_t);\nfigure();\nplot(academy_plot_t,academy_plot_y);\ntitle(\"Five cycles per second\");\nxlabel(\"Time (s)\");\nylabel(\"Amplitude\")", result: "A labelled sine-wave figure appears in Figures." }],
    },
    Lesson {
        group: "4. DSP & Filters", title: "FFT and power spectral density",
        introduction: "fft(array) returns a complex spectrum. abs(spectrum) gives its magnitude; ifft(spectrum) reconstructs the real signal. psd(signal,rate,segment_length) estimates a one-sided Welch power spectral density.",
        notes: &["PSD results have two columns: frequency in Hz and power per Hz. Choose a segment length no longer than the signal.", "Tools → Spectral Analyzer provides the same analysis with interactive controls."],
        examples: &[Example { title: "Analyze a two-tone signal", code: "academy_fs = 1000;\nacademy_dsp_t = (0:999) ./ academy_fs;\nacademy_signal = sin(2*pi*50*academy_dsp_t) + 0.3*sin(2*pi*200*academy_dsp_t);\nacademy_fft = fft(academy_signal);\nacademy_restored = ifft(academy_fft);\nacademy_psd = psd(academy_signal,academy_fs,256);\nfigure();\nplot(academy_psd(:,1),academy_psd(:,2));\ntitle(\"Welch power spectrum\");\nxlabel(\"Frequency (Hz)\");\nylabel(\"Power / Hz\")", result: "The spectrum shows energy near 50 Hz and 200 Hz." }],
    },
    Lesson {
        group: "4. DSP & Filters", title: "Butterworth and FIR filtering",
        introduction: "Butterworth designs return second-order sections (SOS), with six coefficients per row. sosfilt(signal,sos) applies the filter. FIR designs return a vector of taps for filter(signal,taps).",
        notes: &["Cutoff frequencies must lie between zero and half the sample rate. Butterworth orders range from 1 to 8.", "Tools → Filter Design offers low-pass, high-pass, and band-pass designs and response previews. NexusLab's filter argument order is signal first."],
        examples: &[Example { title: "Remove a high-frequency component", code: "academy_filter_fs = 1000;\nacademy_filter_t = (0:999) ./ academy_filter_fs;\nacademy_filter_x = sin(2*pi*40*academy_filter_t) + 0.3*sin(2*pi*250*academy_filter_t);\nacademy_sos = butter_lowpass(4,100,academy_filter_fs);\nacademy_filtered = sosfilt(academy_filter_x,academy_sos);\nacademy_response = freqz(academy_sos,academy_filter_fs);\nfigure();\nplot(academy_filter_t,academy_filtered);\ntitle(\"Butterworth low-pass output\");\nxlabel(\"Time (s)\");\nylabel(\"Amplitude\")", result: "The 40 Hz component remains; the 250 Hz component is attenuated." }, Example { title: "Create FIR taps", code: "academy_taps = fir_lowpass(63,100,1000);\nacademy_impulse = [1 zeros(1,127)];\nacademy_fir_output = filter(academy_impulse,academy_taps);\nplot(academy_fir_output)", result: "The impulse response follows the FIR coefficients." }],
    },
];

#[derive(Default)]
pub(super) struct Academy {
    lesson: usize,
    reference: bool,
}
impl NexusApp {
    pub(super) fn academy_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("NexusAcademy");
            self.panel_button(ui, Panel::Tutorial);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .selectable_label(self.academy.reference, "Reference")
                    .clicked()
                {
                    self.academy.reference = true;
                }
                if ui
                    .selectable_label(!self.academy.reference, "Tutorials")
                    .clicked()
                {
                    self.academy.reference = false;
                }
            });
        });
        ui.separator();
        let mut run = None;
        egui::SidePanel::left("academy_contents")
            .resizable(true)
            .default_width(215.0)
            .width_range(160.0..=300.0)
            .frame(
                egui::Frame::new()
                    .inner_margin(8)
                    .fill(theme::palette(ui.ctx()).background),
            )
            .show_inside(ui, |ui| {
                theme::label(ui, "CONTENTS");
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("academy_toc_scroll")
                    .show(ui, |ui| {
                        for group in [
                            "1. Getting Started",
                            "2. Vectors & Matrices",
                            "3. Plotting Data",
                            "4. DSP & Filters",
                        ] {
                            egui::CollapsingHeader::new(group)
                                .default_open(true)
                                .show(ui, |ui| {
                                    for (index, lesson) in
                                        LESSONS.iter().enumerate().filter(|(_, l)| l.group == group)
                                    {
                                        if ui
                                            .selectable_label(
                                                !self.academy.reference
                                                    && self.academy.lesson == index,
                                                lesson.title,
                                            )
                                            .clicked()
                                        {
                                            self.academy.lesson = index;
                                            self.academy.reference = false;
                                        }
                                    }
                                });
                        }
                    });
            });
        egui::CentralPanel::default().frame(egui::Frame::new().inner_margin(16).fill(theme::palette(ui.ctx()).surface)).show_inside(ui,|ui| {
            egui::ScrollArea::vertical().id_salt(("academy_lesson",self.academy.lesson,self.academy.reference)).auto_shrink([false,false]).show(ui,|ui| {
                        ui.set_max_width(ui.available_width().min(900.0));
                if self.academy.reference {
                    ui.heading("Command reference");
                    ui.add_space(8.0);
                    ui.label("NexusLab implements a MATLAB-style numerical subset. Use the tutorials for examples of the supported syntax and function signatures.");
                    ui.add_space(12.0);
                    for section in nexus_script::HELP.split("\n\n").skip(1) {
                        ui.label(section); ui.add_space(10.0);
                    }
                    ui.separator();
                    ui.label("Compatibility: real, two-dimensional arrays; single-output functions; complex FFT vectors and eigenvalues. MATLAB/Octave control-flow syntax, multi-output assignment, sparse arrays, and complex matrix algebra are not implemented. Use Rhai for loops and conditionals.");
                } else {
                    let lesson=&LESSONS[self.academy.lesson];
                    ui.label(RichText::new(lesson.group).small().color(theme::palette(ui.ctx()).muted));
                    ui.heading(lesson.title);
                    ui.add_space(12.0);
                    ui.label(lesson.introduction);
                    for (i,example) in lesson.examples.iter().enumerate() {
                        ui.add_space(18.0);
                        ui.label(RichText::new(example.title).strong());
                        ui.add_space(6.0);
                        theme::group(ui).show(ui,|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("MATLAB-style").small().color(theme::palette(ui.ctx()).muted));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                                    if ui.add_enabled(!self.busy,egui::Button::new("Try it")).on_hover_text("Execute this example in the Command Window").clicked() { run=Some(example.code); }
                                    if crate::icons::button(ui,crate::icons::Icon::Copy,"Copy example",true).clicked() { ui.ctx().copy_text(example.code.to_owned()); }
                                });
                            });
                            ui.separator();
                            egui::ScrollArea::horizontal().id_salt(("academy_code",i)).show(ui,|ui| {
                                let code=editor::highlight(example.code,false,ui.visuals().dark_mode,self.preferences.code_font_size);
                                ui.add(egui::Label::new(code).selectable(true).wrap_mode(egui::TextWrapMode::Extend));
                            });
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new(example.result).color(theme::palette(ui.ctx()).muted));
                    }
                    ui.add_space(18.0); ui.separator();
                    ui.label(RichText::new("Notes").strong());
                    for note in lesson.notes { ui.horizontal_top(|ui| { ui.label("•"); ui.label(*note); }); }
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(self.academy.lesson>0,egui::Button::new("Previous")).clicked() { self.academy.lesson-=1; }
                        if ui.add_enabled(self.academy.lesson+1<LESSONS.len(),egui::Button::new("Next lesson")).clicked() { self.academy.lesson+=1; }
                    });
                }
            });
        });
        if let Some(code) = run {
            self.input = code.to_owned();
            self.open_panel(Panel::Command);
            self.execute();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_example_runs_independently_and_in_a_shared_workspace() {
        let mut shared = Session::new();
        for lesson in LESSONS {
            for example in lesson.examples {
                let mut fresh = Session::new();
                fresh
                    .evaluate(example.code)
                    .unwrap_or_else(|e| panic!("{} / {}: {e}", lesson.title, example.title));
                shared
                    .evaluate(example.code)
                    .unwrap_or_else(|e| panic!("shared {}: {e}", example.title));
                nexus_script::workspace::validate(&fresh.variables).unwrap();
            }
        }
        assert!(shared.variables["academy_pinv_error"].scalar().unwrap() < 1e-10);
        assert!(shared.variables["academy_residual"].scalar().unwrap() < 1e-10);
        assert_eq!(shared.variables["academy_eigenvalues"].shape(), (2, 1));
    }
}
