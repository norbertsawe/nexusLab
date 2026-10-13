use eframe::egui::{self, Color32, Stroke};
#[derive(Clone, Copy)]
pub enum Icon {
    NewFile,
    Copy,
    Book,
    Close,
    Pause,
    Stop,
    PopOut,
    Dock,
    Save,
    Fit,
    Trash,
    History,
    Expand,
    Folder,
    Refresh,
    Up,
    Apply,
    Plot,
    Run,
    Search,
}
pub fn button(ui: &mut egui::Ui, icon: Icon, tooltip: &str, enabled: bool) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .min_size(egui::vec2(24.0, 24.0))
            .frame(false),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tooltip));
    let rect = response.rect;
    let visuals = ui.style().interact(&response);
    if response.hovered() && enabled {
        ui.painter().rect(
            rect,
            2,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    let color = if enabled {
        visuals.fg_stroke.color
    } else {
        Color32::GRAY
    };
    let p = ui.painter();
    let stroke = Stroke::new(1.0_f32, color);
    let origin = rect.center() - egui::vec2(8.0, 8.0);
    let pos = |x: f32, y: f32| origin + egui::vec2(x, y);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pos(a.0, a.1), pos(b.0, b.1)], stroke);
    };
    let box_at = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            egui::Rect::from_min_size(pos(x, y), egui::vec2(w, h)),
            2,
            stroke,
            egui::StrokeKind::Inside,
        );
    };
    match icon {
        Icon::NewFile => {
            box_at(3.0, 1.0, 10.0, 14.0);
            line((5.0, 8.0), (11.0, 8.0));
            line((8.0, 5.0), (8.0, 11.0));
        }
        Icon::Copy => {
            box_at(1.0, 1.0, 9.0, 10.0);
            box_at(5.0, 5.0, 9.0, 10.0);
        }
        Icon::Book => {
            box_at(1.0, 2.0, 14.0, 12.0);
            line((8.0, 2.0), (8.0, 14.0));
            line((3.0, 5.0), (6.0, 5.0));
            line((10.0, 5.0), (13.0, 5.0));
        }
        Icon::Close => {
            line((4.0, 4.0), (12.0, 12.0));
            line((12.0, 4.0), (4.0, 12.0));
        }
        Icon::Pause => {
            line((5.0, 2.0), (5.0, 14.0));
            line((11.0, 2.0), (11.0, 14.0));
        }
        Icon::Stop => box_at(3.0, 3.0, 10.0, 10.0),
        Icon::PopOut => {
            box_at(1.0, 5.0, 10.0, 10.0);
            line((7.0, 9.0), (14.0, 2.0));
            line((9.0, 2.0), (14.0, 2.0));
            line((14.0, 2.0), (14.0, 7.0));
        }
        Icon::Dock => {
            box_at(1.0, 1.0, 14.0, 14.0);
            line((8.0, 3.0), (8.0, 11.0));
            line((4.0, 7.0), (8.0, 11.0));
            line((12.0, 7.0), (8.0, 11.0));
            line((4.0, 13.0), (12.0, 13.0));
        }
        Icon::Save => {
            box_at(1.0, 1.0, 14.0, 14.0);
            box_at(4.0, 1.0, 7.0, 5.0);
            box_at(4.0, 9.0, 8.0, 6.0);
        }
        Icon::Trash => {
            line((1.0, 4.0), (15.0, 4.0));
            box_at(4.0, 1.0, 8.0, 3.0);
            box_at(3.0, 4.0, 10.0, 11.0);
            line((6.0, 7.0), (6.0, 12.0));
            line((10.0, 7.0), (10.0, 12.0));
        }
        Icon::Fit | Icon::Expand => {
            for (x, y, dx, dy) in [
                (1.0, 1.0, 5.0, 5.0),
                (15.0, 1.0, -5.0, 5.0),
                (1.0, 15.0, 5.0, -5.0),
                (15.0, 15.0, -5.0, -5.0),
            ] {
                line((x, y), (x + dx, y));
                line((x, y), (x, y + dy));
            }
        }
        Icon::History => {
            p.circle_stroke(pos(8.0, 8.0), 6.0, stroke);
            line((8.0, 4.0), (8.0, 8.0));
            line((8.0, 8.0), (11.0, 10.0));
        }
        Icon::Refresh => {
            let points = (0..18)
                .map(|i| {
                    let a = std::f32::consts::TAU * i as f32 / 22.0;
                    pos(8.0 + 6.0 * a.cos(), 8.0 + 6.0 * a.sin())
                })
                .collect();
            p.add(egui::Shape::line(points, stroke));
            line((14.0, 8.0), (14.0, 3.0));
            line((14.0, 8.0), (9.0, 8.0));
        }
        Icon::Folder => {
            box_at(1.0, 4.0, 14.0, 11.0);
            box_at(1.0, 1.0, 7.0, 4.0);
        }
        Icon::Up => {
            line((8.0, 14.0), (8.0, 2.0));
            line((3.0, 7.0), (8.0, 2.0));
            line((13.0, 7.0), (8.0, 2.0));
        }
        Icon::Apply => {
            line((2.0, 8.0), (6.0, 12.0));
            line((6.0, 12.0), (14.0, 3.0));
        }
        Icon::Plot => {
            line((1.0, 1.0), (1.0, 15.0));
            line((1.0, 15.0), (15.0, 15.0));
            line((3.0, 11.0), (7.0, 5.0));
            line((7.0, 5.0), (10.0, 9.0));
            line((10.0, 9.0), (15.0, 2.0));
        }
        Icon::Run => {
            p.add(egui::Shape::convex_polygon(
                vec![pos(4.0, 2.0), pos(14.0, 8.0), pos(4.0, 14.0)],
                color,
                stroke,
            ));
        }
        Icon::Search => {
            p.circle_stroke(pos(6.0, 6.0), 5.0, stroke);
            line((10.0, 10.0), (15.0, 15.0));
        }
    }
    response.on_hover_text(tooltip)
}
