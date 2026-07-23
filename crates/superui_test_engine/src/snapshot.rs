//! PNG baseline snapshot engine for `toHaveScreenshot`.
//!
//! Stores baselines under `<dir>/__snapshots__/<spec_file>/<name>-<platform>.png`.
//! On first run (or `--update`) the baseline is written and the assertion passes.
//! On subsequent runs the actual frame is diffed pixel-by-pixel; a mismatch
//! (diff ratio > `max_diff_ratio`) writes `<name>-<platform>.actual.png` and
//! `<name>-<platform>.diff.png` alongside the baseline and returns `Err`.

use std::path::{Path, PathBuf};

/// Per-channel RGBA delta (0-255) below which a pixel is considered unchanged.
/// Used both for the diff-ratio computation and the diff-image highlight loop.
const PIXEL_TOLERANCE: u8 = 4;

pub struct SnapshotConfig {
    pub dir: PathBuf,
    pub update: bool,
    pub max_diff_ratio: f64,
    pub platform: String,
}

/// Canonical path for a snapshot baseline.
/// Name has optional `.png` suffix stripped; platform tag is appended.
pub fn baseline_path(cfg: &SnapshotConfig, spec_file: &str, name: &str) -> PathBuf {
    let stem = name.trim_end_matches(".png");
    cfg.dir
        .join("__snapshots__")
        .join(spec_file)
        .join(format!("{stem}-{}.png", cfg.platform))
}

/// Fraction of pixels in `a` and `b` that differ by more than `tol` in any
/// RGBA channel.  Returns 1.0 if the slices have different lengths or are empty.
pub fn diff_ratio(a: &[u8], b: &[u8], tol: u8) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 1.0;
    }
    let pixels = a.len() / 4;
    let mut diff = 0usize;
    for i in 0..pixels {
        let o = i * 4;
        let differs = (0..4).any(|c| a[o + c].abs_diff(b[o + c]) > tol);
        if differs {
            diff += 1;
        }
    }
    diff as f64 / pixels as f64
}

/// Compare `actual` RGBA pixels to the stored baseline.
///
/// - If `cfg.update` is true OR no baseline exists: write the baseline and
///   return `Ok(())`.
/// - Otherwise: load the baseline, compute `diff_ratio`, return `Ok(())` if
///   within tolerance, or `Err(msg)` (writing `-actual`/`-diff` PNGs) on
///   mismatch.
pub fn match_screenshot(
    cfg: &SnapshotConfig,
    spec_file: &str,
    name: &str,
    width: u32,
    height: u32,
    actual: &[u8],
) -> Result<(), String> {
    let path = baseline_path(cfg, spec_file, name);

    let write_png = |p: &Path, w: u32, h: u32, data: &[u8]| -> Result<(), String> {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // image 0.25: save_buffer takes `impl Into<ExtendedColorType>`;
        // `image::ColorType` implements `From<ColorType> for ExtendedColorType`,
        // so this conversion is implicit.
        image::save_buffer(p, data, w, h, image::ColorType::Rgba8)
            .map_err(|e| e.to_string())
    };

    if cfg.update || !path.exists() {
        write_png(&path, width, height, actual)?;
        return Ok(());
    }

    let base = image::open(&path)
        .map_err(|e| e.to_string())?
        .to_rgba8();

    if base.width() != width || base.height() != height {
        // Write actual so the user can inspect it.
        let actual_path = path.with_extension("actual.png");
        write_png(&actual_path, width, height, actual)?;
        return Err(format!(
            "screenshot {name}: size mismatch: baseline {}x{} vs actual {}x{}",
            base.width(),
            base.height(),
            width,
            height,
        ));
    }

    let ratio = diff_ratio(base.as_raw(), actual, PIXEL_TOLERANCE);
    if ratio <= cfg.max_diff_ratio {
        Ok(())
    } else {
        // Write actual PNG.
        let actual_path = path.with_extension("actual.png");
        write_png(&actual_path, width, height, actual)?;

        // Write diff PNG: white pixels where channels differ beyond tolerance.
        let mut diff = vec![0u8; actual.len()];
        for i in 0..(actual.len() / 4) {
            let o = i * 4;
            let differs =
                (0..4).any(|c| base.as_raw()[o + c].abs_diff(actual[o + c]) > PIXEL_TOLERANCE);
            let v = if differs { 255u8 } else { 0u8 };
            diff[o] = v;
            diff[o + 1] = v;
            diff[o + 2] = v;
            diff[o + 3] = 255;
        }
        let diff_path = path.with_extension("diff.png");
        write_png(&diff_path, width, height, &diff)?;

        Err(format!(
            "screenshot {name}: diff ratio {ratio:.4} > {:.4}",
            cfg.max_diff_ratio
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::diff_ratio;

    #[test]
    fn identical_images_have_zero_diff() {
        let a = vec![10u8; 400];
        assert_eq!(diff_ratio(&a, &a, 0), 0.0);
    }

    #[test]
    fn one_changed_pixel_reports_fraction() {
        let mut a = vec![0u8; 4 * 4]; // 4 RGBA pixels
        let mut b = a.clone();
        b[0] = 255; // change pixel 0 red channel
        let r = diff_ratio(&a, &b, 0);
        assert!((r - 0.25).abs() < 1e-9, "got {r}");
        let _ = &mut a;
    }

    #[test]
    fn mismatched_lengths_report_full_diff() {
        let a = vec![0u8; 8];
        let b = vec![0u8; 4];
        assert_eq!(diff_ratio(&a, &b, 0), 1.0);
    }

    #[test]
    fn tolerance_suppresses_small_differences() {
        let a = vec![100u8; 4]; // 1 pixel
        let mut b = a.clone();
        b[0] = 103; // diff = 3, within tol=4
        assert_eq!(diff_ratio(&a, &b, 4), 0.0);
        b[0] = 105; // diff = 5, exceeds tol=4
        assert_eq!(diff_ratio(&a, &b, 4), 1.0);
    }
}
