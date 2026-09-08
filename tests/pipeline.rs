//! Integration tests for the imaging core.
//!
//! The detection/geometry tests that used to live here were retired with the classical detector.
//! What survives is the export invariant, which is independent of routing: every JPEG CSP writes
//! carries an embedded sRGB ICC profile and decodes back at the size it was written.

use csp::core::exporter::save;
use csp::core::preprocessor as load;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn saved_jpeg_embeds_icc_and_reads_back_at_size() {
    for name in ["packshot_white.jpg", "packshot_tan.jpg", "hardshadow.jpg"] {
        let loaded = load::load_image(&fixture(name)).unwrap();
        let tmp = std::env::temp_dir().join(format!("csp_test_icc_{name}"));
        save::save_jpeg_srgb(&loaded.rgb, &tmp, 95).unwrap();

        let bytes = std::fs::read(&tmp).unwrap();
        assert!(
            has_app2_icc(&bytes),
            "{name}: output JPEG must carry an APP2 ICC profile"
        );

        let back = image::open(&tmp).unwrap();
        assert_eq!(back.width(), loaded.rgb.width(), "{name}: width drifted");
        assert_eq!(back.height(), loaded.rgb.height(), "{name}: height drifted");
        let _ = std::fs::remove_file(&tmp);
    }
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
