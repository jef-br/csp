//! Batch product-image repositioner.
//!
//! Default (no args): watches the desktop `CSP-INPUT` folder, writes to `CSP-OUTPUT`. With two path
//! args (`<input_dir> <output_dir>`) it runs a plain batch — used for development and testing on
//! non-Windows hosts.

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
    if args.len() == 2 {
        let input = PathBuf::from(&args[0]);
        let output = PathBuf::from(&args[1]);
        let summary = app::batch::run(&input, &output);
        println!(
            "{} ok, {} failed, {:.2}s",
            summary.ok, summary.failed, summary.seconds
        );
        return;
    }
    app::run_default();
}
