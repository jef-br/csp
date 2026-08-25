//! Windows message-box alerts and UI-language detection.

use super::i18n::Lang;
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

/// Show a simple informational message box.
pub fn info(title: &str, body: &str) {
    let title_w = wide(title);
    let body_w = wide(body);
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            body_w.as_ptr(),
            title_w.as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Detect the user's UI language, mapping the primary language id to a supported `Lang`.
pub fn ui_language() -> Lang {
    let langid = unsafe { GetUserDefaultUILanguage() };
    let primary = langid & 0x03ff;
    let code = match primary {
        0x0a => "es",
        0x0c => "fr",
        0x13 => "nl",
        0x10 => "it",
        0x07 => "de",
        _ => "en",
    };
    Lang::from_code(code)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
