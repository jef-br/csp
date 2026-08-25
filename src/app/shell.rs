//! Default (double-click) flow: find the desktop input folder, process, alert on completion.

use super::i18n;
use super::{alert, batch, desktop};

const INPUT_FOLDER: &str = "jb-input";
const OUTPUT_FOLDER: &str = "jb-output";

pub fn run() {
    let lang = alert::ui_language();
    let desktop = desktop::desktop_dir();
    let input = desktop.join(INPUT_FOLDER);
    let output = desktop.join(OUTPUT_FOLDER);

    // Create the input folder on first run and ask the user to drop images in.
    if !input.exists() {
        let _ = std::fs::create_dir_all(&input);
        alert::info(i18n::app_title(), &i18n::drop_images_prompt(lang, INPUT_FOLDER));
        return;
    }

    let has_images = has_any_image(&input);
    if !has_images {
        alert::info(i18n::app_title(), &i18n::drop_images_prompt(lang, INPUT_FOLDER));
        return;
    }

    let summary = batch::run(&input, &output);
    alert::info(
        i18n::app_title(),
        &i18n::batch_finished(lang, summary.seconds, summary.ok, summary.failed),
    );
}

fn has_any_image(dir: &std::path::Path) -> bool {
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
