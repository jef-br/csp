//! Stamps the Windows resources onto `csp.exe`.
//!
//! Only the app icon, which Explorer, the taskbar and the console window all read straight from
//! the binary. `Pepperoni.ico` is committed at the repo root: unlike the model and the runtime it
//! is small, so it lives in the repo rather than being a build-time prerequisite.
fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=Pepperoni.ico");
        winresource::WindowsResource::new()
            .set_icon("Pepperoni.ico")
            .compile()
            .expect("embedding Pepperoni.ico failed");
    }
}
