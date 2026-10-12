use eframe::egui;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    System,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: Theme,
    pub ui_font_size: f32,
    pub code_font_size: f32,
    pub striped_rows: bool,
    pub line_numbers: bool,
    pub show_toolbar: bool,
    pub show_status_bar: bool,
    pub restore_layout: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            ui_font_size: 12.0,
            code_font_size: 13.0,
            striped_rows: true,
            line_numbers: true,
            show_toolbar: true,
            show_status_bar: true,
            restore_layout: true,
        }
    }
}
impl Preferences {
    pub fn dark(&self, ctx: &egui::Context) -> bool {
        match self.theme {
            Theme::Light => false,
            Theme::Dark => true,
            Theme::System => ctx.system_theme() == Some(egui::Theme::Dark),
        }
    }
    pub fn normalize(&mut self) {
        self.ui_font_size = if self.ui_font_size.is_finite() {
            self.ui_font_size.clamp(10.0, 18.0)
        } else {
            12.0
        };
        self.code_font_size = if self.code_font_size.is_finite() {
            self.code_font_size.clamp(10.0, 24.0)
        } else {
            13.0
        };
    }
    pub fn load() -> Self {
        let mut p = path()
            .and_then(|p| nexus_script::workspace::read_limited(&p, 65536).ok())
            .and_then(|b| serde_json::from_slice::<Self>(&b).ok())
            .unwrap_or_default();
        p.normalize();
        p
    }
    pub fn save(&self) -> Result<(), String> {
        let Some(path) = path() else {
            return Ok(());
        };
        std::fs::create_dir_all(path.parent().ok_or("Invalid settings location")?)
            .map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        nexus_script::workspace::atomic_write(&path, &bytes).map_err(|e| e.to_string())
    }
}
fn path() -> Option<PathBuf> {
    crate::documents::recovery_path().map(|p| p.with_file_name("preferences.json"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_settings_handle_missing_fields_and_invalid_sizes() {
        let mut p: Preferences =
            serde_json::from_str(r#"{"theme":"dark","ui_font_size":999,"code_font_size":-2}"#)
                .unwrap();
        p.normalize();
        assert_eq!(p.theme, Theme::Dark);
        assert_eq!(p.ui_font_size, 18.0);
        assert_eq!(p.code_font_size, 10.0);
        assert!(p.restore_layout && p.line_numbers);
        assert_eq!(
            serde_json::from_slice::<Preferences>(&serde_json::to_vec(&p).unwrap()).unwrap(),
            p
        );
    }
}
