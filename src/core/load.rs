//! Image loading: EXIF orientation applied, alpha composited onto white.
//!
//! Mirrors `process_images.py:load_image`. Returns the flattened RGB image plus, when the source
//! carried real transparency, the alpha channel — the detector uses it as an exact fast path.

use image::{DynamicImage, GrayImage, ImageReader, RgbImage};
use std::path::Path;

pub struct Loaded {
    pub rgb: RgbImage,
    pub alpha: Option<GrayImage>,
}

pub fn load_image(path: &Path) -> Result<Loaded, String> {
    let reader = ImageReader::open(path)
        .map_err(|e| format!("open: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("format: {e}"))?;

    let mut decoder = reader.into_decoder().map_err(|e| format!("decode: {e}"))?;
    let orientation = image::ImageDecoder::orientation(&mut decoder)
        .unwrap_or(image::metadata::Orientation::NoTransforms);

    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| format!("decode: {e}"))?;
    img.apply_orientation(orientation);

    let has_alpha = img.color().has_alpha();
    if has_alpha {
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        let mut rgb = RgbImage::new(w, h);
        let mut alpha = GrayImage::new(w, h);
        for (dst, a, src) in itertools_zip(&mut rgb, &mut alpha, &rgba) {
            let af = src[3] as f32 / 255.0;
            // Composite over white: out = src*a + 255*(1-a).
            dst[0] = (src[0] as f32 * af + 255.0 * (1.0 - af)).round() as u8;
            dst[1] = (src[1] as f32 * af + 255.0 * (1.0 - af)).round() as u8;
            dst[2] = (src[2] as f32 * af + 255.0 * (1.0 - af)).round() as u8;
            a[0] = src[3];
        }
        let carries_transparency = alpha.iter().any(|&v| v < 250);
        Ok(Loaded {
            rgb,
            alpha: if carries_transparency { Some(alpha) } else { None },
        })
    } else {
        Ok(Loaded {
            rgb: img.to_rgb8(),
            alpha: None,
        })
    }
}

// Small local zip over the three buffers to keep the composite loop readable without a dependency.
fn itertools_zip<'a>(
    rgb: &'a mut RgbImage,
    alpha: &'a mut GrayImage,
    rgba: &'a image::RgbaImage,
) -> impl Iterator<
    Item = (
        &'a mut image::Rgb<u8>,
        &'a mut image::Luma<u8>,
        &'a image::Rgba<u8>,
    ),
> {
    rgb.pixels_mut()
        .zip(alpha.pixels_mut())
        .zip(rgba.pixels())
        .map(|((r, a), s)| (r, a, s))
}
