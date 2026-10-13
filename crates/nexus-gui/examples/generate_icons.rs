//! Regenerate checked-in PNG/ICO assets from the canonical SVG.
//! cargo run -p nexus-gui --example generate_icons
use std::{error::Error, fs, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let svg = fs::read(directory.join("nexuslab.svg"))?;
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default())?;
    let mut frames = Vec::new();
    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let mut pixmap =
            resvg::tiny_skia::Pixmap::new(size, size).ok_or("icon allocation failed")?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(size as f32 / 256.0, size as f32 / 256.0),
            &mut pixmap.as_mut(),
        );
        let png = pixmap.encode_png()?;
        if size == 256 {
            fs::write(directory.join("nexuslab-256.png"), &png)?;
        }
        frames.push((size, png));
    }
    // ICO directory followed by individually compressed PNG frames. Windows
    // selects the closest size for Explorer, title bars, and taskbar scaling.
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(frames.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * frames.len();
    for (size, png) in &frames {
        ico.extend_from_slice(&[*size as u8, *size as u8, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in frames {
        ico.extend_from_slice(&png);
    }
    fs::write(directory.join("nexuslab.ico"), ico)?;
    println!("Updated assets/nexuslab-256.png and assets/nexuslab.ico");
    Ok(())
}
