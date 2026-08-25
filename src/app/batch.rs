//! Batch processing: enumerate inputs, process each, delete on success, leave failures in place.

use csp::core;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

pub struct Summary {
    pub ok: usize,
    pub failed: usize,
    pub seconds: f64,
}

const SUPPORTED: &[&str] = &["jpg", "jpeg", "png", "bmp", "tif", "tiff", "webp"];

/// Process every supported image in `input` into `output`, in parallel across cores. Successfully
/// processed originals are deleted; failures are left in `input` untouched.
pub fn run(input: &Path, output: &Path) -> Summary {
    let start = Instant::now();
    let _ = std::fs::create_dir_all(output);

    let ok = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);
    collect(input).par_iter().for_each(|path| {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
        let out_path = output.join(format!("{stem}.jpg"));
        match core::process_file(path, &out_path) {
            Ok(()) => {
                ok.fetch_add(1, Ordering::Relaxed);
                let _ = std::fs::remove_file(path);
            }
            Err(e) => {
                failed.fetch_add(1, Ordering::Relaxed);
                eprintln!("failed: {} — {e}", path.display());
            }
        }
    });

    Summary {
        ok: ok.load(Ordering::Relaxed),
        failed: failed.load(Ordering::Relaxed),
        seconds: (start.elapsed().as_secs_f64() * 100.0).round() / 100.0,
    }
}

fn collect(input: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(input) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        if let Some(ext) = ext {
            if SUPPORTED.contains(&ext.as_str()) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
