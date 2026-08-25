//! Application shell: folder discovery, localized prompts, batch orchestration, completion alert.

pub mod batch;
pub mod i18n;

#[cfg(windows)]
mod console;
#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod shell;

/// Entry point when launched with no arguments (the double-click path).
#[cfg(windows)]
pub fn run_default() {
    shell::run();
}

/// Non-Windows fallback: the default folder flow is Windows-only; direct callers use the
/// `<input_dir> <output_dir>` argument form instead.
#[cfg(not(windows))]
pub fn run_default() {
    eprintln!("usage: csp <input_dir> <output_dir>");
}
