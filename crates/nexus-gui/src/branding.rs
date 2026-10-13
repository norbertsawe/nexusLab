use eframe::egui;
use std::sync::{Arc, OnceLock};

pub fn window_icon() -> Arc<egui::IconData> {
    static ICON: OnceLock<Arc<egui::IconData>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(
            eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/nexuslab-256.png"))
                .expect("checked-in NexusLab icon must be a valid PNG"),
        )
    })
    .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_icon_has_rgba_pixels_and_all_windows_sizes() {
        let icon = window_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        let ico = include_bytes!("../../../assets/nexuslab.ico");
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 7, 0]);
        for entry in ico[6..6 + 7 * 16].as_chunks::<16>().0 {
            let len = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            let png = image::load_from_memory(&ico[offset..offset + len]).unwrap();
            let size = if entry[0] == 0 { 256 } else { entry[0] as u32 };
            assert_eq!((png.width(), png.height()), (size, size));
        }
    }
}
