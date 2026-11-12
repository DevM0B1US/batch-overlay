//! Pure image overlay engine.
//!
//! This module has zero Tauri dependencies so it can be unit-tested in
//! isolation (see the test crate in `scripts/overlay-engine-test`).

use std::io::Cursor;

use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, GenericImageView, ImageEncoder, ImageFormat, RgbaImage};

/// How the template is placed on top of the base image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Template at its native size (scaled down only if larger than the base
    /// image), centered.
    Center,
    /// Template stretched to exactly the base image size (aspect ratio ignored).
    Stretch,
    /// Template scaled uniformly to the largest size that fits inside the base
    /// image, centered.
    Fit,
}

impl Placement {
    pub fn parse(value: &str) -> Option<Placement> {
        match value {
            "center" => Some(Placement::Center),
            "stretch" => Some(Placement::Stretch),
            "fit" => Some(Placement::Fit),
            _ => None,
        }
    }
}

/// Output file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Png,
    Jpeg,
}

impl OutFormat {
    pub fn parse(value: &str) -> Option<OutFormat> {
        match value {
            "png" => Some(OutFormat::Png),
            "jpeg" | "jpg" => Some(OutFormat::Jpeg),
            _ => None,
        }
    }

    /// File extension used for output files.
    pub fn extension(self) -> &'static str {
        match self {
            OutFormat::Png => "png",
            OutFormat::Jpeg => "jpg",
        }
    }
}

/// Downscale `img` so it fits inside `max_w` x `max_h`, preserving aspect
/// ratio. Never upscales.
pub fn resize_to_fit(img: &DynamicImage, max_w: u32, max_h: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || max_w == 0 || max_h == 0 {
        return img.clone();
    }
    if w <= max_w && h <= max_h {
        return img.clone();
    }
    let scale = f64::min(max_w as f64 / w as f64, max_h as f64 / h as f64);
    let nw = ((w as f64 * scale).round() as u32).max(1);
    let nh = ((h as f64 * scale).round() as u32).max(1);
    img.resize_exact(nw, nh, FilterType::Lanczos3)
}

/// Compute the drawn size of the template on a `base_w` x `base_h` image.
pub fn template_size(
    placement: Placement,
    tmpl_w: u32,
    tmpl_h: u32,
    base_w: u32,
    base_h: u32,
) -> (u32, u32) {
    if tmpl_w == 0 || tmpl_h == 0 || base_w == 0 || base_h == 0 {
        return (1, 1);
    }
    match placement {
        Placement::Stretch => (base_w, base_h),
        Placement::Fit => {
            let scale = f64::min(base_w as f64 / tmpl_w as f64, base_h as f64 / tmpl_h as f64);
            let w = ((tmpl_w as f64 * scale).round() as u32).clamp(1, base_w);
            let h = ((tmpl_h as f64 * scale).round() as u32).clamp(1, base_h);
            (w, h)
        }
        Placement::Center => {
            if tmpl_w <= base_w && tmpl_h <= base_h {
                (tmpl_w, tmpl_h)
            } else {
                let scale = f64::min(base_w as f64 / tmpl_w as f64, base_h as f64 / tmpl_h as f64);
                let w = ((tmpl_w as f64 * scale).round() as u32).clamp(1, base_w);
                let h = ((tmpl_h as f64 * scale).round() as u32).clamp(1, base_h);
                (w, h)
            }
        }
    }
}

/// Apply `template` on top of `base` with proper alpha blending.
pub fn apply_overlay(base: &DynamicImage, template: &DynamicImage, placement: Placement) -> DynamicImage {
    let (base_w, base_h) = base.dimensions();
    let (tmpl_w, tmpl_h) = template.dimensions();
    let (draw_w, draw_h) = template_size(placement, tmpl_w, tmpl_h, base_w, base_h);

    let mut canvas: RgbaImage = base.to_rgba8();
    let top: RgbaImage = if draw_w != tmpl_w || draw_h != tmpl_h {
        template.resize_exact(draw_w, draw_h, FilterType::Lanczos3).to_rgba8()
    } else {
        template.to_rgba8()
    };

    let dx = (base_w.saturating_sub(draw_w) / 2) as i64;
    let dy = (base_h.saturating_sub(draw_h) / 2) as i64;
    image::imageops::overlay(&mut canvas, &top, dx, dy);

    DynamicImage::ImageRgba8(canvas)
}

/// Encode a composed image. `quality` (0-100) applies to JPEG; PNG is lossless
/// and ignores it.
pub fn encode_image(img: &DynamicImage, format: OutFormat, quality: u8) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    match format {
        OutFormat::Png => {
            img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
                .map_err(|e| format!("PNG encode failed: {e}"))?;
        }
        OutFormat::Jpeg => {
            // JPEG has no alpha channel: flatten onto RGB.
            let rgb = img.to_rgb8();
            let encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.clamp(0, 100));
            encoder
                .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
        }
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn gradient_base(w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let v = ((x * 255) / w.max(1)) as u8;
                img.put_pixel(x, y, Rgba([v, v, v, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    /// Fully opaque-when-composited semi-transparent red template.
    fn alpha_template(w: u32, h: u32) -> DynamicImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba([255, 0, 0, 128]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn parse_enums() {
        assert_eq!(Placement::parse("center"), Some(Placement::Center));
        assert_eq!(Placement::parse("stretch"), Some(Placement::Stretch));
        assert_eq!(Placement::parse("fit"), Some(Placement::Fit));
        assert_eq!(Placement::parse("nope"), None);
        assert_eq!(OutFormat::parse("png"), Some(OutFormat::Png));
        assert_eq!(OutFormat::parse("jpeg"), Some(OutFormat::Jpeg));
        assert_eq!(OutFormat::parse("jpg"), Some(OutFormat::Jpeg));
        assert_eq!(OutFormat::parse("webp"), None);
        assert_eq!(OutFormat::Png.extension(), "png");
        assert_eq!(OutFormat::Jpeg.extension(), "jpg");
    }

    #[test]
    fn center_keeps_base_size_and_blends_alpha() {
        let base = gradient_base(100, 60);
        let tmpl = alpha_template(40, 20);
        let out = apply_overlay(&base, &tmpl, Placement::Center);
        assert_eq!(out.dimensions(), (100, 60));

        // Center of the template region: 50% blend of red over mid-gray.
        let p = out.get_pixel(50, 30);
        assert!((p[0] as i32 - 191).abs() <= 3, "red channel was {}", p[0]);
        assert!((p[2] as i32 - 63).abs() <= 3, "blue channel was {}", p[2]);
        assert!(
            p[3] >= 254,
            "base is opaque so output alpha stays ~255, was {}",
            p[3]
        );

        // Outside the template region: untouched base gradient.
        let q = out.get_pixel(5, 5);
        let expected = ((5 * 255) / 100) as i32;
        assert!((q[0] as i32 - expected).abs() <= 3);
    }

    #[test]
    fn stretch_fills_base_ignoring_aspect() {
        let base = gradient_base(80, 50);
        let tmpl = alpha_template(10, 30);
        let out = apply_overlay(&base, &tmpl, Placement::Stretch);
        assert_eq!(out.dimensions(), (80, 50));
        // Every pixel must be tinted red now.
        for x in [0, 40, 79] {
            for y in [0, 25, 49] {
                let p = out.get_pixel(x, y);
                assert!(p[0] > 100, "pixel ({x},{y}) not tinted: {:?}", p);
            }
        }
    }

    #[test]
    fn fit_preserves_aspect_and_centers() {
        let base = gradient_base(100, 100);
        let tmpl = alpha_template(50, 25);
        let out = apply_overlay(&base, &tmpl, Placement::Fit);
        assert_eq!(out.dimensions(), (100, 100));
        // 50x25 scaled by 2 -> 100x50, centered vertically (rows 25..75).
        let inside = out.get_pixel(50, 50);
        let outside = out.get_pixel(50, 10);
        assert!(inside[0] > outside[0], "row 50 should be tinted, row 10 not");
    }

    #[test]
    fn center_scales_down_oversized_template() {
        let base = gradient_base(100, 100);
        let tmpl = alpha_template(300, 300);
        let out = apply_overlay(&base, &tmpl, Placement::Center);
        assert_eq!(out.dimensions(), (100, 100));
        let p = out.get_pixel(50, 50);
        assert!(p[0] > 100, "center should be tinted after downscale");
    }

    #[test]
    fn resize_to_fit_downscales_only() {
        let img = gradient_base(200, 100);
        let out = resize_to_fit(&img, 100, 100);
        assert_eq!(out.dimensions(), (100, 50));
        let unchanged = resize_to_fit(&img, 400, 400);
        assert_eq!(unchanged.dimensions(), (200, 100));
        let tiny_bound = resize_to_fit(&img, 10, 400);
        assert_eq!(tiny_bound.dimensions(), (10, 5));
    }

    #[test]
    fn encode_roundtrip() {
        let img = gradient_base(64, 32);

        let png = encode_image(&img, OutFormat::Png, 90).unwrap();
        assert_eq!(&png[0..4], b"\x89PNG");
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!(decoded.dimensions(), (64, 32));

        let jpeg = encode_image(&img, OutFormat::Jpeg, 85).unwrap();
        assert_eq!(&jpeg[0..2], b"\xFF\xD8");
        let decoded = image::load_from_memory(&jpeg).unwrap();
        assert_eq!(decoded.dimensions(), (64, 32));
    }

    #[test]
    fn jpeg_quality_changes_file_size() {
        let img = gradient_base(256, 256);
        let high = encode_image(&img, OutFormat::Jpeg, 100).unwrap();
        let low = encode_image(&img, OutFormat::Jpeg, 20).unwrap();
        assert!(high.len() > low.len(), "q100 ({}B) should exceed q20 ({}B)", high.len(), low.len());
    }
}
