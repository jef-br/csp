//! Integration tests for the imaging core: detection sanity, sizing envelope, product-not-distorted
//! invariant, ICC embedding, and the alpha fast path.

use csp::core::config::{MAX_SIZE, MIN_SIZE};
use csp::core::types::DetectionKind;
use csp::core::{detect, load, reposition, save};
use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

#[test]
fn packshot_box_is_not_whole_frame() {
    let loaded = load::load_image(&fixture("packshot_white.jpg")).unwrap();
    let det = detect::detect(&loaded.rgb, loaded.alpha.as_ref());
    assert!(matches!(det.kind, DetectionKind::Subject), "expected a subject box, got {:?}", det.kind);
    // The product occupies well under the whole frame.
    let frame = loaded.rgb.width() as i64 * loaded.rgb.height() as i64;
    assert!(det.box_.area() < frame, "box should be smaller than the frame");
    assert!(det.box_.w > 10 && det.box_.h > 10, "box should be non-degenerate");
}

#[test]
fn output_is_square_and_in_size_envelope() {
    for name in ["packshot_white.jpg", "packshot_tan.jpg", "hardshadow.jpg"] {
        let loaded = load::load_image(&fixture(name)).unwrap();
        let out = reposition(&loaded.rgb, loaded.alpha.as_ref());
        assert_eq!(out.width(), out.height(), "{name}: output must be square");
        let side = out.width();
        assert!(
            (MIN_SIZE..=MAX_SIZE).contains(&side),
            "{name}: side {side} out of [{MIN_SIZE},{MAX_SIZE}]"
        );
    }
}

#[test]
fn saved_jpeg_embeds_icc_and_reads_back_square() {
    let loaded = load::load_image(&fixture("packshot_tan.jpg")).unwrap();
    let out = reposition(&loaded.rgb, loaded.alpha.as_ref());
    let tmp = std::env::temp_dir().join("csp_test_icc.jpg");
    save::save_jpeg_srgb(&out, &tmp, 95).unwrap();

    let bytes = std::fs::read(&tmp).unwrap();
    assert!(has_app2_icc(&bytes), "output JPEG must carry an APP2 ICC profile");

    let back = image::open(&tmp).unwrap();
    assert_eq!(back.width(), back.height());
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn alpha_fast_path_finds_the_opaque_box() {
    // Transparent frame with a centered opaque red box.
    let (w, h) = (400u32, 300u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 0]));
    let (bx, by, bw, bh) = (120u32, 80u32, 160u32, 140u32);
    for y in by..by + bh {
        for x in bx..bx + bw {
            img.put_pixel(x, y, Rgba([200, 30, 30, 255]));
        }
    }
    let tmp = std::env::temp_dir().join("csp_test_alpha.png");
    img.save(&tmp).unwrap();

    let loaded = load::load_image(&tmp).unwrap();
    assert!(loaded.alpha.is_some(), "loader must surface the alpha channel");
    let det = detect::detect(&loaded.rgb, loaded.alpha.as_ref());
    // Box should tightly match the opaque region (within a couple of px).
    assert!((det.box_.x - bx as i32).abs() <= 2, "x off: {}", det.box_.x);
    assert!((det.box_.y - by as i32).abs() <= 2, "y off: {}", det.box_.y);
    assert!((det.box_.w - bw as i32).abs() <= 2, "w off: {}", det.box_.w);
    assert!((det.box_.h - bh as i32).abs() <= 2, "h off: {}", det.box_.h);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn product_band_is_not_distorted_by_fill() {
    // A synthetic product (sharp checker box) on a flat sweep: after repositioning, the product's
    // aspect ratio must be preserved — the fill only grows background, never the product.
    let (w, h) = (600u32, 400u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([245, 245, 245, 255]));
    let (bx, by, bw, bh) = (240u32, 120u32, 120u32, 160u32); // 3:4 product
    for y in by..by + bh {
        for x in bx..bx + bw {
            let c = if ((x / 8) + (y / 8)) % 2 == 0 { 20 } else { 90 };
            img.put_pixel(x, y, Rgba([c, c, c, 255]));
        }
    }
    let tmp = std::env::temp_dir().join("csp_test_prod.png");
    img.save(&tmp).unwrap();

    let loaded = load::load_image(&tmp).unwrap();
    let det = detect::detect(&loaded.rgb, loaded.alpha.as_ref());
    // Detected aspect ratio should be close to the true 3:4 (0.75), proving the product wasn't
    // stretched during detection/scaling.
    let ar = det.box_.w as f64 / det.box_.h as f64;
    assert!((ar - 0.75).abs() < 0.2, "product aspect ratio drifted: {ar:.3}");
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn tall_subject_with_margin_keeps_full_height() {
    // A tall red bar that does NOT touch the frame edges (has background all around) must keep its
    // full height and pad to a square — the product is never cropped or distorted.
    let (w, h) = (400u32, 700u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([250, 250, 250, 255]));
    let (bx, by, bw, bh) = (160u32, 60u32, 80u32, 580u32);
    for y in by..by + bh {
        for x in bx..bx + bw {
            img.put_pixel(x, y, Rgba([200, 40, 40, 255]));
        }
    }
    let tmp = std::env::temp_dir().join("csp_test_tall.png");
    img.save(&tmp).unwrap();

    let loaded = load::load_image(&tmp).unwrap();
    let det = detect::detect(&loaded.rgb, loaded.alpha.as_ref());
    assert!(
        (det.box_.h - bh as i32).abs() < 60,
        "full height should be kept: got {} want ~{}",
        det.box_.h,
        bh
    );
    let out = reposition(&loaded.rgb, loaded.alpha.as_ref());
    assert_eq!(out.width(), out.height(), "output must be square");
    let _ = std::fs::remove_file(&tmp);
}

fn has_app2_icc(jpeg: &[u8]) -> bool {
    let mut i = 2usize; // skip SOI
    while i + 4 <= jpeg.len() {
        if jpeg[i] != 0xFF {
            break;
        }
        let marker = jpeg[i + 1];
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        let len = ((jpeg[i + 2] as usize) << 8) | jpeg[i + 3] as usize;
        let seg = &jpeg[i + 4..(i + 2 + len).min(jpeg.len())];
        if marker == 0xE2 && seg.starts_with(b"ICC_PROFILE") {
            return true;
        }
        i += 2 + len;
    }
    false
}
