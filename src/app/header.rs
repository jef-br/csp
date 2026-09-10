//! Startup banner: the ASCII logo in a light box, the product name, version and tagline beside it,
//! then the two standing instruction lines and the rule that closes the block.
//!
//! Hand-tuned on purpose — there is no logic here, only text and padding. Edit `LOGO`, `NAME` and
//! `TAGLINE` freely; `print` is called once at the top of `shell::run`.

use super::i18n::{self, Lang};
use super::theme::{self, BOLD, ORANGE, RESET, WHITE};

/// ASCII logo, one row per line. The box in [`theme::boxed`] adds the frame.
const LOGO: &[&str] = &[
    " _____                             _",
    "|  _  |___ ___ ___ ___ ___ ___ ___|_|",
    "|   __| -_| . | . | -_|  _| . |   | |",
    "|__|  |___|  _|  _|___|_| |___|_|_|_|",
    "          |_| |_|",
];

const NAME: &str = "Post Production Picture Preparation, Program";

/// The three `→` lines beside the name. Product shorthand, kept in English in every UI language.
const TAGLINE: &[&str] = &[
    "→ BridgeBatch but faster",
    "→ Automatic Center & Stretch + CropSquare + 'on white'",
    "→ Save as JPG in sRGB. 800-2000px",
];

/// Gap between the logo box and the text column beside it.
const GUTTER: usize = 3;

/// Row of the logo box the name sits on: past the top border and the logo's first row, so the name
/// lines up with the body of the art. The tagline follows on the rows below.
const TEXT_TOP: usize = 2;

/// Print the banner: logo block, the standing instructions, and the closing rule.
pub fn print(lang: Lang) {
    let rows = theme::boxed(LOGO, WHITE);
    let left = theme::boxed_width(LOGO) + GUTTER;

    let mut text = vec![format!(
        "{BOLD}{ORANGE}{NAME}{}- V{}{RESET}",
        " ".repeat(4),
        env!("CARGO_PKG_VERSION")
    )];
    text.extend(TAGLINE.iter().map(|l| format!("{ORANGE}{l}{RESET}")));

    for (i, row) in rows.iter().enumerate() {
        match i.checked_sub(TEXT_TOP).and_then(|n| text.get(n)) {
            Some(line) => {
                let pad = left - theme::boxed_width(LOGO);
                println!("{row}{}{line}", " ".repeat(pad));
            }
            None => println!("{row}"),
        }
    }

    println!("{WHITE}{}{RESET}", i18n::drop_prompt(lang));
    println!("{WHITE}{}{RESET}", i18n::close_line(lang));
    println!("{}", theme::rule());
}
