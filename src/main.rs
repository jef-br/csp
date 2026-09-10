//! Batch product-image repositioner.
//!
//! Default (no args): watches the desktop `CSP-INPUT` folder, writes to `CSP-OUTPUT` and moves the
//! originals to `CSP-BACKUP`. With path args (`<input_dir> <output_dir> [backup_dir]`) it runs a
//! plain batch — used for development and testing on non-Windows hosts. Without a third arg the
//! backup folder is `CSP-BACKUP` beside the output folder.

mod app;

use std::path::PathBuf;

fn main() {
    // A missing ONNX Runtime or model is an environment fault, not an image outcome: stop before
    // touching a single file rather than processing the whole batch with a dead classifier.
    if let Err(e) = csp::core::preflight() {
        eprintln!("cannot start: {e}");
        std::process::exit(1);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 2 || args.len() == 3 {
        let input = PathBuf::from(&args[0]);
        let output = PathBuf::from(&args[1]);
        let backup = match args.get(2) {
            Some(path) => PathBuf::from(path),
            None => output
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("CSP-BACKUP"),
        };
        let summary = app::batch::run(&input, &output, &backup, &[]);
        println!(
            "{} ok, {} failed, {:.2}s",
            summary.ok, summary.failed, summary.seconds
        );
        return;
    }
    app::run_default();
}
