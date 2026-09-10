//! Default (double-click) flow: find the desktop input folder, process, report in the console.
//!
//! Two screens share one banner. Idle: the input folder in a box, then the "drop images and run me
//! again" callout. Running: the progress bar, then the "CTRL+C to interrupt" callout, then the
//! summary and the output folder. `theme` owns every rule, box and colour used here.

use super::{batch, console, desktop, header, i18n, theme};
use std::io::Write;
use std::path::Path;

const INPUT_FOLDER: &str = "CSP-INPUT";
const OUTPUT_FOLDER: &str = "CSP-OUTPUT";
const BACKUP_FOLDER: &str = "CSP-BACKUP";

/// Left margin of the folder box, matching the indent in `docs`-side layout.
const INDENT: &str = "  ";

pub fn run() {
    console::init();
    let lang = console::ui_language();
    header::print(lang);

    let desktop = desktop::desktop_dir();
    let input = desktop.join(INPUT_FOLDER);
    let output = desktop.join(OUTPUT_FOLDER);
    let backup = desktop.join(BACKUP_FOLDER);

    let created = !input.exists();
    if created {
        let _ = std::fs::create_dir_all(&input);
    }

    // First run / nothing to do: point the user at the input folder and wait.
    if created || !has_any_image(&input) {
        println!();
        print_folder(&input);
        println!();
        print_callout(i18n::ready_hint(lang));
        console::pause();
        return;
    }

    let _ = std::fs::create_dir_all(&output);
    let _ = std::fs::create_dir_all(&backup);
    println!("\n{}{}{}", theme::WHITE, i18n::processing(lang), theme::RESET);

    // The bar is drawn first and the callout under it stays put; `Progress` walks the cursor back
    // over these lines on every redraw, so they have to be handed over rather than printed here.
    let footer = vec![
        String::new(),
        theme::rule(),
        theme::callout(i18n::interrupt_hint(lang)),
        theme::rule(),
    ];
    let summary = batch::run(&input, &output, &backup, &footer);

    println!();
    for line in &summary.failures {
        println!("{}{line}{}", theme::GREY, theme::RESET);
    }
    println!(
        "{}{}{}\n",
        theme::WHITE,
        i18n::finished(lang, summary.seconds, summary.ok, summary.failed),
        theme::RESET
    );
    print_folder(&output);
    println!();
    print_callout(i18n::close_line(lang));
    console::pause();
}

/// The folder path in its own box: magenta, indented, and clickable under Windows Terminal.
fn print_folder(path: &Path) {
    let text = path.display().to_string();
    let paint = format!("{}{}", theme::BOLD, theme::MAGENTA);
    let url = console::folder_url(path);
    for row in theme::boxed_linked(&[text.as_str()], &paint, url.as_deref()) {
        println!("{INDENT}{row}");
    }
}

/// A `⏵` line bracketed by two full-width rules.
fn print_callout(text: &str) {
    println!("{}", theme::rule());
    println!("{}", theme::callout(text));
    println!("{}", theme::rule());
    let _ = std::io::stdout().flush();
}

// True when `dir` has something for batch::run to process: an image directly inside it, or an
// image inside one of its direct subfolders (batch recurses exactly one level — see batch.rs).
fn has_any_image(dir: &Path) -> bool {
    has_direct_image(dir) || has_subfolder_image(dir)
}

fn has_direct_image(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for e in entries.flatten() {
        if let Some(ext) = e.path().extension().and_then(|x| x.to_str()) {
            let ext = ext.to_ascii_lowercase();
            if ["jpg", "jpeg", "png", "bmp", "tif", "tiff", "webp"].contains(&ext.as_str()) {
                return true;
            }
        }
    }
    false
}

fn has_subfolder_image(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries
        .flatten()
        .any(|e| e.path().is_dir() && has_direct_image(&e.path()))
}
