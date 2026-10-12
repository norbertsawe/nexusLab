use super::*;

fn highlight(source: &str, rhai: bool, dark: bool, font_size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let chars: Vec<_> = source.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let start = chars[i].0;
        let c = chars[i].1;
        let mut color = if dark {
            egui::Color32::from_rgb(220, 220, 220)
        } else {
            theme::TEXT
        };
        if c == '%' && !rhai || c == '/' && chars.get(i + 1).is_some_and(|p| p.1 == '/') {
            color = if dark {
                egui::Color32::from_rgb(106, 153, 85)
            } else {
                egui::Color32::from_rgb(0, 128, 0)
            };
            while i < chars.len() && chars[i].1 != '\n' {
                i += 1;
            }
        } else if c == '"' || c == '\'' {
            color = if dark {
                egui::Color32::from_rgb(206, 145, 120)
            } else {
                egui::Color32::from_rgb(163, 21, 21)
            };
            let quote = c;
            i += 1;
            while i < chars.len() && chars[i].1 != quote && chars[i].1 != '\n' {
                if chars[i].1 == '\\' && i + 1 < chars.len() {
                    i += 1;
                }
                i += 1;
            }
            if i < chars.len() && chars[i].1 == quote {
                i += 1;
            }
        } else if c.is_ascii_digit() {
            color = if dark {
                egui::Color32::from_rgb(181, 206, 168)
            } else {
                egui::Color32::from_rgb(9, 134, 88)
            };
            i += 1;
            while i < chars.len()
                && (chars[i].1.is_ascii_digit() || matches!(chars[i].1, '.' | 'e' | 'E'))
            {
                i += 1;
            }
        } else if c.is_ascii_alphabetic() || c == '_' {
            i += 1;
            while i < chars.len() && (chars[i].1.is_ascii_alphanumeric() || chars[i].1 == '_') {
                i += 1;
            }
            let end = chars.get(i).map_or(source.len(), |p| p.0);
            let word = &source[start..end];
            if matches!(
                word,
                "let"
                    | "for"
                    | "while"
                    | "if"
                    | "else"
                    | "return"
                    | "fn"
                    | "true"
                    | "false"
                    | "in"
                    | "break"
                    | "continue"
            ) {
                color = if dark {
                    egui::Color32::from_rgb(86, 156, 214)
                } else {
                    egui::Color32::from_rgb(0, 0, 192)
                };
            } else if chars.get(i).is_some_and(|p| p.1 == '(') {
                color = if dark {
                    egui::Color32::from_rgb(220, 220, 170)
                } else {
                    egui::Color32::from_rgb(0, 88, 160)
                };
            }
        } else {
            i += 1;
        }
        let end = chars.get(i).map_or(source.len(), |p| p.0);
        job.append(
            &source[start..end],
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(font_size),
                color,
                line_height: Some(font_size + 7.0),
                ..Default::default()
            },
        );
    }
    job.wrap.max_width = f32::INFINITY;
    job
}
impl NexusApp {
    pub(super) fn editor_ui(&mut self, ui: &mut egui::Ui) {
        let font_size = self.preferences.code_font_size;
        let line_numbers = self.preferences.line_numbers;
        ui.horizontal(|ui| {
            ui.heading("Script Editor");
            self.panel_button(ui, Panel::Editor);
        });
        ui.add_space(6.0);
        let mut close = None;
        let mut active = None;
        egui::ScrollArea::horizontal()
            .id_salt("editor_tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (index, document) in self.documents.tabs.iter().enumerate() {
                        ui.push_id(document.id, |ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        index == self.documents.active,
                                        format!(
                                            "{}{}",
                                            document.name,
                                            if document.dirty() { " *" } else { "" }
                                        ),
                                    )
                                    .clicked()
                                {
                                    active = Some(index);
                                }
                                if ui.small_button("×").on_hover_text("Close tab").clicked() {
                                    close = Some(index);
                                }
                            })
                        });
                    }
                    if ui.small_button("+").clicked() {
                        active = Some(self.documents.tabs.len());
                    }
                })
            });
        if let Some(index) = active {
            if index == self.documents.tabs.len() {
                self.new_document("untitled.nx", "", false);
            } else {
                self.documents.active = index;
                self.selection.clear();
                self.cursor = 0;
            }
        }
        if let Some(index) = close {
            self.request_close_tab(index);
        }
        ui.horizontal(|ui| {
            if crate::icons::button(
                ui,
                crate::icons::Icon::Run,
                "Run current script (F5)",
                !self.busy,
            )
            .clicked()
            {
                self.run_editor(false);
            }
            if crate::icons::button(
                ui,
                crate::icons::Icon::Run,
                "Run selected code (Ctrl+Shift+Enter)",
                !self.busy && !self.selection.trim().is_empty(),
            )
            .clicked()
            {
                self.run_editor(true);
            }
            if crate::icons::button(
                ui,
                crate::icons::Icon::Search,
                "Find and replace (Ctrl+F)",
                true,
            )
            .clicked()
            {
                self.find_open = !self.find_open;
            }
            if !self.figures.is_empty()
                && ui
                    .button(format!("View figures ({})", self.figures.len()))
                    .clicked()
            {
                self.set_view(View::Figures);
            }
            let doc = self.documents.current_mut();
            egui::ComboBox::from_id_salt("editor_language")
                .selected_text(if doc.rhai { "Rhai" } else { "MATLAB / Nexus" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut doc.rhai, false, "MATLAB / Nexus");
                    ui.selectable_value(&mut doc.rhai, true, "Rhai");
                });
        });
        let mut jump = None;
        if self.find_open {
            ui.horizontal(|ui| {
                ui.label("Find");
                ui.add(egui::TextEdit::singleline(&mut self.find).desired_width(155.0));
                if ui.button("Next").clicked() && !self.find.is_empty() {
                    let text = &self.documents.current().text;
                    let start = if text.is_char_boundary(self.find_offset.min(text.len())) {
                        self.find_offset.min(text.len())
                    } else {
                        0
                    };
                    let found = text[start..]
                        .find(&self.find)
                        .map(|i| start + i)
                        .or_else(|| text.find(&self.find));
                    if let Some(at) = found {
                        let end = at + self.find.len();
                        jump = Some((text[..at].chars().count(), text[..end].chars().count()));
                        self.find_offset = end;
                    } else {
                        self.notify("No matches in this document.".into(), true);
                    }
                }
                ui.label("Replace");
                ui.add(egui::TextEdit::singleline(&mut self.replace).desired_width(140.0));
                if ui.button("Replace all").clicked() && !self.find.is_empty() {
                    let doc = self.documents.current_mut();
                    doc.text = doc.text.replace(&self.find, &self.replace);
                    self.find_offset = 0;
                }
                if ui.small_button("×").clicked() {
                    self.find_open = false;
                }
            });
        }
        let doc = self.documents.current();
        let prefix: String = doc.text.chars().take(self.cursor).collect();
        let line = prefix.chars().filter(|&c| c == '\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(
                    doc.path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "Not saved".into()),
                )
                .small()
                .color(theme::palette(ui.ctx()).muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("Ln {line}, Col {column}"))
                        .small()
                        .color(theme::palette(ui.ctx()).muted),
                );
            });
        });
        ui.add_space(5.0);
        let id = egui::Id::new(("script_editor", self.documents.current().id));
        let available = ui.available_size();
        let mut cursor = None;
        let mut selection = None;
        theme::group(ui)
            .fill(theme::palette(ui.ctx()).surface)
            .inner_margin(10)
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt(("editor_scroll", self.documents.current().id))
                    .auto_shrink([false; 2])
                    .max_height((available.y - 24.0).max(80.0))
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            let doc = self.documents.current_mut();
                            let lines = doc.text.chars().filter(|&c| c == '\n').count() + 1;
                            let numbers = (1..=lines)
                                .map(|i| format!("{i:>4}"))
                                .collect::<Vec<_>>()
                                .join("\n");
                            let mut gutter = egui::text::LayoutJob::default();
                            gutter.append(
                                &numbers,
                                0.0,
                                egui::TextFormat {
                                    font_id: egui::FontId::monospace(font_size),
                                    color: theme::palette(ui.ctx()).muted,
                                    line_height: Some(font_size + 7.0),
                                    ..Default::default()
                                },
                            );
                            if line_numbers {
                                ui.label(gutter);
                            }
                            let rhai = doc.rhai;
                            let mut layouter = |ui: &egui::Ui, text: &str, _width: f32| {
                                ui.fonts(|fonts| {
                                    fonts.layout_job(highlight(
                                        text,
                                        rhai,
                                        ui.visuals().dark_mode,
                                        font_size,
                                    ))
                                })
                            };
                            let mut output = egui::TextEdit::multiline(&mut doc.text)
                                .id(id)
                                .frame(false)
                                .code_editor()
                                .font(egui::TextStyle::Monospace)
                                .desired_width((available.x - 85.0).max(300.0))
                                .desired_rows(20)
                                .lock_focus(true)
                                .layouter(&mut layouter)
                                .show(ui);
                            if let Some((start, end)) = jump {
                                output.state.cursor.set_char_range(Some(
                                    egui::text::CCursorRange::two(
                                        egui::text::CCursor::new(start),
                                        egui::text::CCursor::new(end),
                                    ),
                                ));
                                output.state.store(ui.ctx(), output.response.id);
                                output.response.request_focus();
                            }
                            if let Some(range) = output.cursor_range {
                                cursor = Some(range.primary.ccursor.index);
                                selection = Some(
                                    doc.text
                                        .chars()
                                        .skip(range.as_sorted_char_range().start)
                                        .take(range.as_sorted_char_range().len())
                                        .collect::<String>(),
                                );
                            }
                        });
                    });
            });
        if let Some(cursor) = cursor {
            self.cursor = cursor;
        }
        if let Some(selection) = selection {
            self.selection = selection;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn syntax_highlighting_preserves_unicode_and_source() {
        let source = "% measured Δ\nx = sin(1.25); title('Ω');";
        let job = highlight(source, false, false, 13.0);
        assert_eq!(job.text, source);
        assert!(job.sections.len() > 5);
    }
}
