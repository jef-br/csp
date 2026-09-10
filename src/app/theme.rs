//! Console styling shared by the shell: the palette, the full-width rule, and the light boxes the
//! header, the folder path and the callout lines are drawn in.
//!
//! Hand-tuned on purpose — there is no logic here beyond padding. Colours are 256-colour ANSI so
//! they look the same in conhost and Windows Terminal; `console::init` turns VT processing on
//! before any of this is printed.

pub const ORANGE: &str = "\x1b[38;5;208m";
pub const MAGENTA: &str = "\x1b[38;5;201m";
pub const GREY: &str = "\x1b[38;5;240m";
pub const WHITE: &str = "\x1b[38;5;252m";
pub const ORANGE_DIM: &str = "\x1b[38;5;94m";
pub const BOLD: &str = "\x1b[1m";
pub const RESET: &str = "\x1b[0m";

/// Width of every horizontal rule. Wide enough for the longest localized callout line, narrow
/// enough not to wrap in a default 100-column console.
pub const WIDTH: usize = 97;

/// The full-width separator that brackets each block.
pub fn rule() -> String {
    format!("{GREY}{}{RESET}", "─".repeat(WIDTH))
}

/// The `⏵` marker that opens a callout line.
pub fn callout(text: &str) -> String {
    format!("{ORANGE}⏵{RESET}{WHITE}{text}{RESET}")
}

/// Draw `lines` inside a light rounded box, every line padded to the widest one. `paint` colours
/// the text; the frame is always grey. Returns one string per box row — top border, one row per
/// input line, bottom border — so callers can print a second column beside it.
///
/// Width is counted in `char`s: every line that reaches here is ASCII art or a Windows path.
pub fn boxed(lines: &[&str], paint: &str) -> Vec<String> {
    boxed_linked(lines, paint, None)
}

/// [`boxed`], with every text row turned into an OSC-8 hyperlink to `url`. The frame stays plain,
/// so a terminal that ignores OSC-8 still shows an intact box.
pub fn boxed_linked(lines: &[&str], paint: &str, url: Option<&str>) -> Vec<String> {
    let inner = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 2;
    let mut rows = Vec::with_capacity(lines.len() + 2);
    rows.push(format!("{GREY}╭{}╮{RESET}", "─".repeat(inner)));
    for line in lines {
        let pad = inner - 1 - line.chars().count();
        let text = match url {
            // BEL-terminated OSC-8 rather than ESC-backslash: both are valid and Windows Terminal
            // takes either, and BEL keeps this string free of backslash pile-ups.
            Some(url) => format!("\x1b]8;;{url}\x07{paint}{line}{RESET}\x1b]8;;\x07"),
            None => format!("{paint}{line}{RESET}"),
        };
        rows.push(format!("{GREY}│{RESET} {text}{}{GREY}│{RESET}", " ".repeat(pad)));
    }
    rows.push(format!("{GREY}╰{}╯{RESET}", "─".repeat(inner)));
    rows
}

/// Visible width of a box drawn by [`boxed`] over the same lines.
pub fn boxed_width(lines: &[&str]) -> usize {
    lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 4
}
