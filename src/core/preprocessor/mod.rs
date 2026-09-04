//! Preprocessor: decode, apply EXIF orientation, extract alpha, composite onto white, convert to
//! sRGB.

use image::{DynamicImage, GrayImage, ImageReader, RgbImage};
use std::path::Path;
use tintbox::format::decode::TYPE_RGB_8;
use tintbox::profile::{virtuals::build_srgb_profile, ColorSpace, Profile, RenderingIntent};
use tintbox::transform::Transform;

pub struct Loaded {
    pub rgb: RgbImage,
    pub alpha: Option<GrayImage>,
}

// One pass: decode + apply EXIF orientation + extract alpha + composite onto white + convert to sRGB.
pub fn load_image(path: &Path) -> Result<Loaded, String> {
    let reader = ImageReader::open(path)
        .map_err(|e| format!("open: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("format: {e}"))?;

    let mut decoder = reader.into_decoder().map_err(|e| format!("decode: {e}"))?;
    let orientation = image::ImageDecoder::orientation(&mut decoder)
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let icc_profile = image::ImageDecoder::icc_profile(&mut decoder).unwrap_or(None);

    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| format!("decode: {e}"))?;
    img.apply_orientation(orientation);

    let has_alpha = img.color().has_alpha();
    let (mut rgb, alpha) = if has_alpha {
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        let mut rgb = RgbImage::new(w, h);
        let mut alpha = GrayImage::new(w, h);
        for (dst, a, src) in itertools_zip(&mut rgb, &mut alpha, &rgba) {
            let af = src[3] as f32 / 255.0;
            dst[0] = (src[0] as f32 * af + 255.0 * (1.0 - af)).round() as u8;
            dst[1] = (src[1] as f32 * af + 255.0 * (1.0 - af)).round() as u8;
            dst[2] = (src[2] as f32 * af + 255.0 * (1.0 - af)).round() as u8;
            a[0] = src[3];
        }
        let carries_transparency = alpha.iter().any(|&v| v < 250);
        (rgb, if carries_transparency { Some(alpha) } else { None })
    } else {
        (img.to_rgb8(), None)
    };

    // A source tagged with a non-sRGB RGB profile gets colour-managed into sRGB here, before any
    // downstream pixel math (shot classification, background fill) runs on it. An untagged image,
    // or one already tagged sRGB, is left untouched — same as today.
    if let Some(icc) = icc_profile.as_deref() {
        rgb = convert_to_srgb(rgb, icc);
    }

    Ok(Loaded { rgb, alpha })
}

// Colour-manage `rgb` from its embedded ICC profile into sRGB via tintbox (a pure-Rust,
// bit-identical-to-lcms2 CMM). Anything that doesn't parse as an RGB profile, or that tintbox
// can't build a transform for, leaves `rgb` untouched — same fallback as no profile at all.
fn convert_to_srgb(rgb: RgbImage, icc: &[u8]) -> RgbImage {
    let (w, h) = (rgb.width(), rgb.height());
    let Ok(source) = Profile::open(icc) else {
        return rgb;
    };
    if source.header().color_space != ColorSpace::Rgb {
        return rgb;
    }
    let Ok(target) = Profile::from_writable(&build_srgb_profile()) else {
        return rgb;
    };
    let Ok(xform) = Transform::new_simple_with_formats(
        &source,
        &target,
        RenderingIntent::RelativeColorimetric,
        true, // black-point compensation
        TYPE_RGB_8,
        TYPE_RGB_8,
    ) else {
        return rgb;
    };

    let n_pixels = w as usize * h as usize;
    let src = rgb.into_raw();
    let mut out = vec![0u8; src.len()];
    xform.do_transform(&src, &mut out, n_pixels);
    RgbImage::from_raw(w, h, out).expect("transform preserves buffer size")
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
