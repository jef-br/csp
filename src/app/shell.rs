//! Default (double-click) flow: find the desktop input folder, process, report in the console.

use super::{batch, console, desktop, i18n};
use std::io::Write;
use std::path::Path;

const INPUT_FOLDER: &str = "jb-input";
const OUTPUT_FOLDER: &str = "jb-output";

pub fn run() {
    console::init();
    let lang = console::ui_language();
    let desktop = desktop::desktop_dir();
    let input = desktop.join(INPUT_FOLDER);
    let output = desktop.join(OUTPUT_FOLDER);

    let created = !input.exists();
    if created {
        let _ = std::fs::create_dir_all(&input);
    }

    // First run / nothing to do: point the user at the input folder and wait.
    if created || !has_any_image(&input) {
        println!("\n{}\n", i18n::drop_prompt(lang));
        println!("    {}\n", console::folder_link(&input));
        print_close(lang, &input);
        console::pause();
        return;
    }

    let _ = std::fs::create_dir_all(&output);
    println!("\n{}", i18n::processing(lang));
    let summary = batch::run(&input, &output);
    println!("\n{}", i18n::finished(lang, summary.seconds, summary.ok, summary.failed));
    println!("    {}\n", console::folder_link(&output));
    print_close(lang, &input);
    console::pause();
}

fn print_close(lang: i18n::Lang, input: &Path) {
    let line = i18n::close_line(lang).replacen("{}", &console::folder_link(input), 1);
    println!("{line}");
    let _ = std::io::stdout().flush();
}

fn has_any_image(dir: &Path) -> bool {
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
