//! Startup banner: unicode-art logo + name + one-line summary.
//!
//! Hand-tuned on purpose — there is no logic here, only text. Edit `LOGO`, `NAME`, and `SUMMARY`
//! freely. `print` is called once at the top of `shell::run`.

/// Unicode-art logo. Placeholder block spelling CSP — replace with the final art.
const LOGO: &str = "\
   ▄████▄    ██████   ▄█████▄
  ██     ▀  ██        ██    ██
  ██        ▀██████▄  ██▄▄▄██▀
  ██     ▄        ██  ██▀▀▀▀
   ▀████▀   ██████▀   ██                
";

const NAME: &str = "csp — Command-line Sprite Packer";

const SUMMARY: &str = "Batch-cleans product photos: cuts the background, reframes the subject, \
and exports square JPEGs. Drop a folder in, get a folder out.";

/// ANSI kept local so this file stays self-contained.
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[95m";
const RESET: &str = "\x1b[0m";

/// Print the banner to stdout, followed by a blank line.
pub fn print() {
    println!("{CYAN}{LOGO}{RESET}");
    println!("{BOLD}{NAME}{RESET}");
    println!("{DIM}{SUMMARY}{RESET}\n");
}
