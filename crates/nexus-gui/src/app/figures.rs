use super::*;
use egui_plot::{Bar, BarChart, PlotPoint, Points};
#[derive(Clone, Copy)]
pub(super) enum ExportFormat {
    Svg,
    Png,
    Csv,
}

pub(super) fn sampled(series: &nexus_script::PlotSeries) -> Vec<[f64; 2]> {
    let n = series.x.len().min(series.y.len());
    let stride = n.div_ceil(8192).max(1);
    let mut points: Vec<_> = (0..n)
        .step_by(stride)
        .map(|i| [series.x[i], series.y[i]])
        .collect();
    if n > 0 && !(n - 1).is_multiple_of(stride) {
        points.push([series.x[n - 1], series.y[n - 1]]);
    }
    points
}
impl NexusApp {
    pub(super) fn figures_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Figures");
            self.panel_button(ui, Panel::Figures);
            let response = crate::icons::button(
                ui,
                crate::icons::Icon::Save,
                "Export figure (SVG, high-DPI PNG, raw CSV)",
                !self.figures.is_empty() && !self.busy,
            );
            let id = ui.make_persistent_id("figure_export_menu");
            if response.clicked() {
                ui.memory_mut(|m| m.toggle_popup(id));
            }
            egui::popup_below_widget(
                ui,
                id,
                &response,
                egui::PopupCloseBehavior::CloseOnClick,
                |ui| {
                    ui.set_width(240.0);
                    for (format, label) in [
                        (ExportFormat::Svg, "SVG vector figure..."),
                        (ExportFormat::Png, "PNG image (4000 × 2400)..."),
                        (ExportFormat::Csv, "CSV raw series data..."),
                    ] {
                        if ui.button(label).clicked() {
                            self.export_figure(format);
                            ui.close_menu();
                        }
                    }
                },
            );
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            for (id, figure) in &self.figures {
                if ui
                    .selectable_label(
                        self.active_figure == *id,
                        format!("{id} · {}", figure.title),
                    )
                    .clicked()
                {
                    self.active_figure = *id;
                    self.auto_scale = true;
                }
            }
        });
        let Some(figure) = self
            .figures
            .get(&self.active_figure)
            .or_else(|| self.figures.values().next())
        else {
            theme::group(ui).show(ui, |ui| {
                ui.heading("Create a figure");
                ui.label("Run the analysis script, plot a workspace variable, or enter:");
                ui.label(RichText::new("x = 0:0.1:10; y = sin(x); plot(x,y)").monospace());
            });
            return;
        };
        ui.add_space(8.0);
        theme::group(ui).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&figure.title).size(14.0).strong());
                if crate::icons::button(ui, crate::icons::Icon::Fit, "Auto-scale figure", true)
                    .clicked()
                {
                    self.auto_scale = true;
                }
            });
            let colors = if ui.visuals().dark_mode {
                [
                    egui::Color32::from_rgb(77, 163, 255),
                    egui::Color32::from_rgb(106, 191, 105),
                    egui::Color32::from_rgb(183, 144, 219),
                    egui::Color32::from_rgb(232, 179, 97),
                    egui::Color32::from_rgb(239, 127, 155),
                ]
            } else {
                [
                    BLUE,
                    GREEN,
                    egui::Color32::from_rgb(112, 48, 160),
                    egui::Color32::from_rgb(196, 103, 0),
                    egui::Color32::from_rgb(196, 32, 64),
                ]
            };
            let response = Plot::new(("figure", figure.id))
                .height((ui.available_height() - 30.0).max(180.0))
                .legend(Legend::default())
                .show_background(false)
                .show_grid([figure.grid; 2])
                .x_axis_label(&figure.x_label)
                .y_axis_label(&figure.y_label)
                .allow_zoom(false)
                .show_x(false)
                .show_y(false)
                .label_formatter(|_, _| String::new())
                .allow_scroll(false)
                .allow_drag(false)
                .sense(egui::Sense::drag())
                .show(ui, |plot| {
                    if self.auto_scale {
                        plot.set_auto_bounds(egui::Vec2b::TRUE);
                    }
                    for (index, series) in figure.series.iter().enumerate() {
                        let points = sampled(series);
                        let color = colors[index % colors.len()];
                        match series.kind {
                            nexus_script::PlotKind::Line => plot.line(
                                Line::new(&series.name, PlotPoints::from(points))
                                    .color(color)
                                    .width(1.8_f32),
                            ),
                            nexus_script::PlotKind::Scatter => plot.points(
                                Points::new(&series.name, PlotPoints::from(points))
                                    .color(color)
                                    .radius(4.0_f32),
                            ),
                            nexus_script::PlotKind::Bar => {
                                let width = if points.len() > 1 {
                                    (points[1][0] - points[0][0]).abs() * 0.75
                                } else {
                                    0.8
                                };
                                plot.bar_chart(
                                    BarChart::new(
                                        &series.name,
                                        points
                                            .into_iter()
                                            .map(|p| Bar::new(p[0], p[1]).width(width))
                                            .collect(),
                                    )
                                    .color(color),
                                );
                            }
                        }
                    }
                    if plot.response().dragged_by(egui::PointerButton::Middle)
                        || plot.response().dragged_by(egui::PointerButton::Primary)
                    {
                        let delta = plot.pointer_coordinate_drag_delta();
                        plot.translate_bounds(-delta);
                        plot.set_auto_bounds(egui::Vec2b::FALSE);
                    }
                    if plot.response().hovered() {
                        let wheel = plot.ctx().input(|i| {
                            i.events
                                .iter()
                                .filter_map(|e| {
                                    if let egui::Event::MouseWheel {
                                        delta,
                                        modifiers,
                                        unit,
                                    } = e
                                    {
                                        Some((*delta, *modifiers, *unit))
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                        });
                        for (delta, modifiers, unit) in wheel {
                            let scale = match unit {
                                egui::MouseWheelUnit::Point => 1.0,
                                egui::MouseWheelUnit::Line => 40.0,
                                egui::MouseWheelUnit::Page => 400.0,
                            };
                            let factor =
                                ((delta.x + delta.y) * scale * 0.003).clamp(-0.8, 0.8).exp();
                            let zoom = if modifiers.ctrl || modifiers.command {
                                egui::vec2(factor, 1.0)
                            } else if modifiers.shift {
                                egui::vec2(1.0, factor)
                            } else {
                                egui::vec2(factor, factor)
                            };
                            plot.zoom_bounds_around_hovered(zoom);
                            plot.set_auto_bounds(egui::Vec2b::FALSE);
                        }
                        if let Some(point) = plot.pointer_coordinate() {
                            let target = plot.screen_from_plot(point);
                            let nearest = figure
                                .series
                                .iter()
                                .flat_map(|s| {
                                    sampled(s).into_iter().map(move |p| (s.name.as_str(), p))
                                })
                                .min_by(|(_, a), (_, b)| {
                                    let dist = |p: &[f64; 2]| {
                                        plot.screen_from_plot(PlotPoint::new(p[0], p[1]))
                                            .distance_sq(target)
                                    };
                                    dist(a).total_cmp(&dist(b))
                                });
                            return Some((point, nearest));
                        }
                    }
                    None
                });
            if let Some((point, nearest)) = response.inner {
                if let Some(pos) = response.response.hover_pos() {
                    let frame = *response.transform.frame();
                    if frame.contains(pos) {
                        let painter = ui.painter().with_clip_rect(frame);
                        painter.line_segment(
                            [
                                egui::pos2(frame.left(), pos.y),
                                egui::pos2(frame.right(), pos.y),
                            ],
                            egui::Stroke::new(1.0_f32, theme::palette(ui.ctx()).muted),
                        );
                        painter.line_segment(
                            [
                                egui::pos2(pos.x, frame.top()),
                                egui::pos2(pos.x, frame.bottom()),
                            ],
                            egui::Stroke::new(1.0_f32, theme::palette(ui.ctx()).muted),
                        );
                    }
                }
                response.response.on_hover_ui_at_pointer(|ui| {
                    ui.monospace(format!("x = {:.6}   y = {:.6}", point.x, point.y));
                    if let Some((name, p)) = nearest {
                        ui.label(format!("{name}: ({:.6}, {:.6})", p[0], p[1]));
                    }
                    ui.small("Wheel: zoom · Ctrl: X · Shift: Y · Middle drag: pan");
                });
            }
        });
        self.auto_scale = false;
    }
}
pub(super) fn export_bytes(figure: &Figure, format: ExportFormat) -> Result<Vec<u8>, String> {
    match format {
        ExportFormat::Svg => Ok(svg(figure).into_bytes()),
        ExportFormat::Csv => {
            let mut csv = String::from("series,x,y\n");
            for series in &figure.series {
                let name = series.name.replace('"', "\"\"");
                for (x, y) in series.x.iter().zip(&series.y) {
                    csv.push_str(&format!("\"{name}\",{x},{y}\n"));
                }
            }
            Ok(csv.into_bytes())
        }
        ExportFormat::Png => {
            let mut options = resvg::usvg::Options::default();
            options.fontdb_mut().load_system_fonts();
            let tree =
                resvg::usvg::Tree::from_str(&svg(figure), &options).map_err(|e| e.to_string())?;
            let mut pixmap =
                resvg::tiny_skia::Pixmap::new(4000, 2400).ok_or("PNG allocation failed")?;
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(4.0, 4.0),
                &mut pixmap.as_mut(),
            );
            let mut bytes = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut bytes, 4000, 2400);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder.set_pixel_dims(Some(png::PixelDimensions {
                    xppu: 11811,
                    yppu: 11811,
                    unit: png::Unit::Meter,
                }));
                let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
                writer
                    .write_image_data(pixmap.data())
                    .map_err(|e| e.to_string())?;
            }
            Ok(bytes)
        }
    }
}
pub(super) fn svg(figure: &Figure) -> String {
    fn escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
    let points: Vec<_> = figure.series.iter().flat_map(sampled).collect();
    let (mut xmin, mut xmax, mut ymin, mut ymax) = (
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    );
    for point in &points {
        xmin = xmin.min(point[0]);
        xmax = xmax.max(point[0]);
        ymin = ymin.min(point[1]);
        ymax = ymax.max(point[1]);
    }
    if !xmin.is_finite() {
        xmin = 0.0;
        xmax = 1.0;
        ymin = 0.0;
        ymax = 1.0;
    }
    if figure
        .series
        .iter()
        .any(|s| s.kind == nexus_script::PlotKind::Bar)
    {
        ymin = ymin.min(0.0);
        ymax = ymax.max(0.0);
    }
    if xmax == xmin {
        xmax += 1.0;
        xmin -= 1.0;
    }
    if ymax == ymin {
        ymax += 1.0;
        ymin -= 1.0;
    }
    let x = |v: f64| 80.0 + (v - xmin) / (xmax - xmin) * 840.0;
    let y = |v: f64| 510.0 - (v - ymin) / (ymax - ymin) * 420.0;
    let mut svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"600\" viewBox=\"0 0 1000 600\"><rect width=\"1000\" height=\"600\" fill=\"#ffffff\"/><g font-family=\"sans-serif\" fill=\"#202020\"><text x=\"80\" y=\"42\" font-size=\"24\">{}</text><text x=\"500\" y=\"580\" text-anchor=\"middle\">{}</text><text transform=\"translate(24 300) rotate(-90)\" text-anchor=\"middle\">{}</text>",escape(&figure.title),escape(&figure.x_label),escape(&figure.y_label));
    for i in 0..=5 {
        let xv = xmin + (xmax - xmin) * i as f64 / 5.0;
        let yv = ymin + (ymax - ymin) * i as f64 / 5.0;
        svg.push_str(&format!("<path d=\"M {} 90 V 510 M 80 {} H 920\" stroke=\"#d9d9d9\"/><text x=\"{}\" y=\"538\" text-anchor=\"middle\">{:.3}</text><text x=\"70\" y=\"{}\" text-anchor=\"end\">{:.3}</text>",x(xv),y(yv),x(xv),xv,y(yv),yv));
    }
    let colors = ["#0078d7", "#107c10", "#7030a0", "#c46700", "#c42040"];
    for (index, series) in figure.series.iter().enumerate() {
        let color = colors[index % colors.len()];
        let points = sampled(series);
        match series.kind {
            nexus_script::PlotKind::Line => {
                let points = points
                    .iter()
                    .map(|p| format!("{:.2},{:.2}", x(p[0]), y(p[1])))
                    .collect::<Vec<_>>()
                    .join(" ");
                svg.push_str(&format!("<polyline points=\"{points}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"2\"/>"));
            }
            nexus_script::PlotKind::Scatter => {
                for p in points {
                    svg.push_str(&format!(
                        "<circle cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"{color}\"/>",
                        x(p[0]),
                        y(p[1])
                    ));
                }
            }
            nexus_script::PlotKind::Bar => {
                for p in points {
                    svg.push_str(&format!(
                        "<rect x=\"{}\" y=\"{}\" width=\"8\" height=\"{}\" fill=\"{color}\"/>",
                        x(p[0]) - 4.0,
                        y(p[1]).min(y(0.0)),
                        (y(p[1]) - y(0.0)).abs()
                    ));
                }
            }
        }
        svg.push_str(&format!(
            "<text x=\"{}\" y=\"64\" font-size=\"12\" fill=\"{color}\">{}</text>",
            80 + index * 150,
            escape(&series.name)
        ));
    }
    svg.push_str("</g></svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_resolution_and_csv_keep_raw_points() {
        let figure = Figure {
            id: 1,
            title: "Export test".into(),
            x_label: "Time".into(),
            y_label: "Value".into(),
            grid: true,
            series: vec![nexus_script::PlotSeries {
                name: "Raw,\"data\"".into(),
                x: (0..10001).map(|i| i as f64).collect(),
                y: vec![1.0; 10001],
                kind: nexus_script::PlotKind::Line,
            }],
        };
        let csv = String::from_utf8(export_bytes(&figure, ExportFormat::Csv).unwrap()).unwrap();
        assert_eq!(csv.lines().count(), 10002);
        assert!(csv.contains("\"Raw,\"\"data\"\"\",10000,1"));
        let png = export_bytes(&figure, ExportFormat::Png).unwrap();
        let reader = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        assert_eq!((reader.info().width, reader.info().height), (4000, 2400));
        assert_eq!(reader.info().pixel_dims.unwrap().xppu, 11811);
    }
    #[test]
    fn export_escapes_labels_and_keeps_final_sample() {
        let series = nexus_script::PlotSeries {
            name: "Data & fit".into(),
            x: (0..10000).map(|i| i as f64).collect(),
            y: vec![1.0; 10000],
            kind: nexus_script::PlotKind::Line,
        };
        assert_eq!(sampled(&series).last().unwrap()[0], 9999.0);
        let figure = Figure {
            id: 1,
            title: "A < B".into(),
            x_label: "x".into(),
            y_label: "y".into(),
            grid: true,
            series: vec![series],
        };
        let output = svg(&figure);
        assert!(output.contains("A &lt; B"));
        assert!(output.contains("Data &amp; fit"));
        assert!(!output.contains("NaN"));
    }
}
