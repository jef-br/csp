//! Console output helpers: ANSI colour, clickable folder links, UI-language detection, and a
//! "press any key" pause so the window stays open on a double-click launch.

use super::i18n::Lang;
use std::path::Path;
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::System::Console::{
    GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
    STD_OUTPUT_HANDLE,
};

const CYAN: &str = "\x1b[96m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Enable ANSI/virtual-terminal processing so colours and hyperlinks render (Windows 10+).
pub fn init() {
    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0u32;
        if GetConsoleMode(handle, &mut mode) != 0 {
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

/// Detect the user's UI language, mapping the primary language id to a supported `Lang`.
pub fn ui_language() -> Lang {
    let langid = unsafe { GetUserDefaultUILanguage() };
    let code = match langid & 0x03ff {
        0x0a => "es",
        0x0c => "fr",
        0x13 => "nl",
        0x10 => "it",
        0x07 => "de",
        _ => "en",
    };
    Lang::from_code(code)
}

/// A folder path rendered in colour. Under Windows Terminal (which supports OSC-8) it is also a
/// clickable link that opens Explorer; classic conhost gets the plain coloured path (no risk of a
/// stray escape showing).
pub fn folder_link(path: &Path) -> String {
    let display = path.display().to_string();
    let colored = format!("{BOLD}{CYAN}{display}{RESET}");
    if std::env::var_os("WT_SESSION").is_some() {
        let url = file_url(path);
        format!("\x1b]8;;{url}\x1b\\{colored}\x1b]8;;\x1b\\")
    } else {
        colored
    }
}

fn file_url(path: &Path) -> String {
    let s = path.display().to_string().replace('\\', "/").replace(' ', "%20");
    format!("file:///{s}")
}

/// Block until the user presses any key (no Enter required).
pub fn pause() {
    #[link(name = "msvcrt")]
    extern "C" {
        fn _getch() -> i32;
    }
    unsafe {
        let _ = _getch();
    }
}
