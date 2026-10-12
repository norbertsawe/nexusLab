use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle};

pub const BACKGROUND: Color32 = Color32::from_rgb(240, 240, 240);
pub const CARD: Color32 = Color32::WHITE;
pub const BORDER: Color32 = Color32::from_rgb(208, 208, 208);
pub const TEXT: Color32 = Color32::from_rgb(32, 32, 32);
pub const MUTED: Color32 = Color32::from_rgb(96, 96, 96);
pub const BLUE: Color32 = Color32::from_rgb(0, 120, 215);
pub const GREEN: Color32 = Color32::from_rgb(16, 124, 16);
pub const ERROR: Color32 = Color32::from_rgb(176, 0, 32);

#[cfg(windows)]
fn system_color(index: i32, _fallback: Color32) -> Color32 {
    // SAFETY: GetSysColor reads a process-independent system color; these are
    // documented COLOR_* indices and require no handle or allocated resource.
    let color = unsafe { windows_sys::Win32::Graphics::Gdi::GetSysColor(index) };
    Color32::from_rgb(color as u8, (color >> 8) as u8, (color >> 16) as u8)
}
#[cfg(not(windows))]
fn system_color(_index: i32, fallback: Color32) -> Color32 {
    fallback
}

#[cfg(windows)]
fn install_system_fonts(ctx: &egui::Context) {
    let directory = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:/Windows"))
        .join("Fonts");
    let mut fonts = egui::FontDefinitions::default();
    for (name, filenames, family) in [
        (
            "Windows UI",
            ["SegUIVar.ttf", "segoeui.ttf"],
            FontFamily::Proportional,
        ),
        (
            "Windows Code",
            ["CascadiaMono.ttf", "consola.ttf"],
            FontFamily::Monospace,
        ),
    ] {
        if let Some(bytes) = filenames
            .iter()
            .find_map(|filename| std::fs::read(directory.join(filename)).ok())
        {
            fonts
                .font_data
                .insert(name.into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.into());
        }
    }
    ctx.set_fonts(fonts);
}

pub fn install_fonts(ctx: &egui::Context) {
    #[cfg(windows)]
    install_system_fonts(ctx);
    #[cfg(not(windows))]
    let _ = ctx;
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub error: Color32,
}
fn colors(dark: bool) -> Palette {
    if dark {
        Palette {
            background: Color32::from_rgb(30, 30, 30),
            surface: Color32::from_rgb(24, 24, 24),
            border: Color32::from_rgb(56, 56, 56),
            text: Color32::from_rgb(220, 220, 220),
            muted: Color32::from_rgb(155, 160, 168),
            error: Color32::from_rgb(255, 139, 139),
        }
    } else {
        Palette {
            background: BACKGROUND,
            surface: CARD,
            border: BORDER,
            text: TEXT,
            muted: MUTED,
            error: ERROR,
        }
    }
}
pub fn palette(ctx: &egui::Context) -> Palette {
    colors(ctx.style().visuals.dark_mode)
}

pub fn apply(ctx: &egui::Context, preferences: &crate::preferences::Preferences) {
    let dark = preferences.dark(ctx);
    let colors = colors(dark);
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(if dark {
        egui::SystemTheme::Dark
    } else {
        egui::SystemTheme::Light
    }));
    let mut style = (*ctx.style()).clone();
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.override_text_color = None;
    style.visuals.panel_fill = if dark {
        colors.background
    } else {
        system_color(15, BACKGROUND)
    };
    style.visuals.window_fill = style.visuals.panel_fill;
    style.visuals.extreme_bg_color = colors.surface;
    style.visuals.faint_bg_color = if !preferences.striped_rows {
        colors.surface
    } else if dark {
        Color32::from_rgb(35, 38, 43)
    } else {
        Color32::from_rgb(233, 238, 244)
    };
    style.visuals.window_corner_radius = CornerRadius::same(2);
    style.visuals.menu_corner_radius = CornerRadius::same(2);
    style.visuals.window_stroke = Stroke::new(1.0_f32, colors.border);
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.selection.bg_fill = system_color(13, BLUE);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, system_color(14, Color32::WHITE));
    style.visuals.text_cursor.stroke = Stroke::new(1.0_f32, colors.text);
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(2);
        widget.expansion = 0.0;
        widget.fg_stroke = Stroke::new(
            1.0_f32,
            if dark {
                colors.text
            } else {
                system_color(8, TEXT)
            },
        );
        widget.bg_stroke = Stroke::new(1.0_f32, colors.border);
    }
    style.visuals.widgets.noninteractive.bg_fill = colors.surface;
    style.visuals.widgets.noninteractive.weak_bg_fill = colors.background;
    style.visuals.widgets.inactive.bg_fill = if dark {
        Color32::from_rgb(48, 48, 48)
    } else {
        Color32::from_rgb(225, 225, 225)
    };
    style.visuals.widgets.inactive.weak_bg_fill = colors.background;
    style.visuals.widgets.hovered.bg_fill = if dark {
        Color32::from_rgb(48, 61, 76)
    } else {
        Color32::from_rgb(229, 241, 251)
    };
    style.visuals.widgets.hovered.weak_bg_fill = style.visuals.widgets.hovered.bg_fill;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BLUE);
    style.visuals.widgets.active.bg_fill = if dark {
        Color32::from_rgb(39, 65, 92)
    } else {
        Color32::from_rgb(204, 228, 247)
    };
    style.visuals.widgets.active.weak_bg_fill = style.visuals.widgets.active.bg_fill;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, BLUE);
    style.visuals.widgets.open = style.visuals.widgets.hovered;
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.interact_size = egui::vec2(24.0, 24.0);
    style.spacing.combo_width = 140.0;
    style.spacing.menu_margin = egui::Margin::same(3);
    style.spacing.window_margin = egui::Margin::same(8);
    for (text_style, size, family) in [
        (
            TextStyle::Heading,
            preferences.ui_font_size + 2.0,
            FontFamily::Proportional,
        ),
        (
            TextStyle::Body,
            preferences.ui_font_size,
            FontFamily::Proportional,
        ),
        (
            TextStyle::Button,
            preferences.ui_font_size,
            FontFamily::Proportional,
        ),
        (
            TextStyle::Small,
            preferences.ui_font_size - 1.0,
            FontFamily::Proportional,
        ),
        (
            TextStyle::Monospace,
            preferences.code_font_size,
            FontFamily::Monospace,
        ),
    ] {
        style
            .text_styles
            .insert(text_style, FontId::new(size, family));
    }
    ctx.set_style(style);
}

pub fn group(ui: &egui::Ui) -> egui::Frame {
    let colors = palette(ui.ctx());
    egui::Frame::new()
        .fill(colors.surface)
        .corner_radius(2)
        .stroke(Stroke::new(1.0_f32, colors.border))
        .inner_margin(8)
}
pub fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong());
}

#[derive(Clone, Copy)]
pub enum FileIcon {
    Folder,
    Script,
    Data,
    Workspace,
    Config,
    Document,
}
impl FileIcon {
    pub fn for_path(path: &std::path::Path, directory: bool) -> Self {
        if directory {
            return Self::Folder;
        }
        match path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "nx" | "m" | "rhai" | "rs" | "py" | "js" | "ps1" => Self::Script,
            "csv" | "tsv" | "mat" | "npy" => Self::Data,
            "nxw" => Self::Workspace,
            "toml" | "json" | "yaml" | "yml" | "ini" => Self::Config,
            _ => Self::Document,
        }
    }
}

/// A full-width hit target keeps stripes, selection, and icons aligned even
/// when file names are longer than the explorer pane.
pub fn explorer_row(
    ui: &mut egui::Ui,
    name: &str,
    icon: FileIcon,
    index: usize,
    selected: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            name,
        )
    });
    let colors = palette(ui.ctx());
    let dark = ui.visuals().dark_mode;
    let background = if selected {
        if dark {
            Color32::from_rgb(38, 65, 94)
        } else {
            Color32::from_rgb(210, 230, 250)
        }
    } else if response.hovered() {
        if dark {
            Color32::from_rgb(44, 53, 65)
        } else {
            Color32::from_rgb(222, 233, 246)
        }
    } else if index.is_multiple_of(2) {
        colors.surface
    } else {
        ui.visuals().faint_bg_color
    };
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0, background);
    if selected {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height())),
            0,
            BLUE,
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(0.5),
            0,
            Stroke::new(1.0_f32, BLUE),
            egui::StrokeKind::Inside,
        );
    }
    paint_file_icon(&painter, rect.min + egui::vec2(8.0, 5.0), icon, dark);
    let font = TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(name.to_owned(), font, colors.text);
    job.wrap.max_width = (rect.width() - 38.0).max(1.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    painter.galley(
        egui::pos2(rect.left() + 32.0, rect.center().y - galley.size().y / 2.0),
        galley,
        colors.text,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn paint_file_icon(painter: &egui::Painter, origin: egui::Pos2, icon: FileIcon, dark: bool) {
    let pos = |x: f32, y: f32| origin + egui::vec2(x, y);
    let (color, fill) = if dark {
        match icon {
            FileIcon::Folder => (
                Color32::from_rgb(221, 174, 68),
                Color32::from_rgb(72, 56, 31),
            ),
            FileIcon::Script => (
                Color32::from_rgb(88, 161, 221),
                Color32::from_rgb(25, 43, 59),
            ),
            FileIcon::Data => (
                Color32::from_rgb(98, 185, 143),
                Color32::from_rgb(27, 49, 38),
            ),
            FileIcon::Workspace => (
                Color32::from_rgb(173, 141, 216),
                Color32::from_rgb(44, 34, 59),
            ),
            FileIcon::Config | FileIcon::Document => (
                Color32::from_rgb(160, 173, 192),
                Color32::from_rgb(39, 43, 49),
            ),
        }
    } else {
        match icon {
            FileIcon::Folder => (
                Color32::from_rgb(154, 107, 24),
                Color32::from_rgb(255, 218, 124),
            ),
            FileIcon::Script => (
                Color32::from_rgb(27, 98, 165),
                Color32::from_rgb(231, 242, 253),
            ),
            FileIcon::Data => (
                Color32::from_rgb(25, 121, 89),
                Color32::from_rgb(231, 246, 239),
            ),
            FileIcon::Workspace => (
                Color32::from_rgb(106, 72, 163),
                Color32::from_rgb(241, 234, 250),
            ),
            FileIcon::Config => (
                Color32::from_rgb(95, 109, 129),
                Color32::from_rgb(237, 241, 246),
            ),
            FileIcon::Document => (Color32::from_rgb(103, 113, 127), Color32::WHITE),
        }
    };
    let stroke = Stroke::new(1.2_f32, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        painter.line_segment([pos(a.0, a.1), pos(b.0, b.1)], stroke);
    };
    if matches!(icon, FileIcon::Folder) {
        painter.add(egui::Shape::convex_polygon(
            vec![
                pos(1.0, 2.0),
                pos(6.0, 2.0),
                pos(8.0, 4.0),
                pos(15.0, 4.0),
                pos(15.0, 14.0),
                pos(1.0, 14.0),
            ],
            fill,
            stroke,
        ));
        line((1.0, 6.0), (15.0, 6.0));
        return;
    }
    // Folded page silhouette with distinct symbols for scripts, tables,
    // saved workspaces, settings, and ordinary documents.
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos(3.0, 1.0),
            pos(10.0, 1.0),
            pos(14.0, 5.0),
            pos(14.0, 15.0),
            pos(3.0, 15.0),
        ],
        fill,
        stroke,
    ));
    line((10.0, 1.0), (10.0, 5.0));
    line((10.0, 5.0), (14.0, 5.0));
    match icon {
        FileIcon::Script => {
            line((7.0, 7.0), (5.0, 9.0));
            line((5.0, 9.0), (7.0, 11.0));
            line((10.0, 7.0), (12.0, 9.0));
            line((12.0, 9.0), (10.0, 11.0));
        }
        FileIcon::Data => {
            for y in [7.0, 10.0, 13.0] {
                line((5.0, y), (12.0, y));
            }
            for x in [5.0, 8.5, 12.0] {
                line((x, 7.0), (x, 13.0));
            }
        }
        FileIcon::Workspace => {
            for (x, y) in [(5.0, 7.0), (9.0, 7.0), (5.0, 11.0), (9.0, 11.0)] {
                painter.rect_filled(
                    egui::Rect::from_min_size(pos(x, y), egui::vec2(2.5, 2.5)),
                    0,
                    color,
                );
            }
        }
        FileIcon::Config => {
            for (y, x) in [(7.0, 7.0), (10.0, 10.0), (13.0, 8.0)] {
                line((5.0, y), (12.0, y));
                painter.circle_filled(pos(x, y), 1.3, color);
            }
        }
        FileIcon::Document => {
            for y in [7.0, 10.0, 13.0] {
                line((5.0, y), (11.0, y));
            }
        }
        FileIcon::Folder => unreachable!(),
    }
}
