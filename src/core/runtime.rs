//! The ONNX Runtime shared library, carried inside the executable.
//!
//! CSP ships as one file. The BiRefNet weights are already compiled in
//! (`shot_classifier::birefnet`); this module does the same for
//! `onnxruntime.dll`, the last file that used to sit beside the exe.
//!
//! `ort`'s `load-dynamic` needs a real path to `LoadLibrary`, so the
//! embedded copy is written out once to a per-user cache directory
//! (`%LOCALAPPDATA%`, falling back to the temp dir) and the path to that
//! copy is handed to `ort::init_from`. The extraction is
//! keyed on the library's size + a content sample: a rebuild against a
//! different runtime lands in a different directory, and repeat runs reuse
//! the file already there.
//!
//! This exists only while the runtime cannot be linked in — see
//! `Cargo.toml`'s `ort` entry and `docs/ARCHITECTURE.md` §5. When a static
//! ORT >= 1.22 is available, this module and `birefnet::init_runtime` both
//! delete outright.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The ONNX Runtime library, compiled into the executable.
///
/// Gitignored and expected at the repo root, exactly like the `.onnx`
/// weights: a missing file is a *build* failure naming it, not a runtime
/// surprise.
static DYLIB_BYTES: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/onnxruntime.dll"));

const DYLIB_NAME: &str = "pproni.dll";

/// Path to an `onnxruntime.dll` on disk that `ort` can load.
///
/// `ORT_DYLIB_PATH` wins when it points at a real file — the `examples/`
/// dev harness sets it to run against a hand-placed runtime. Otherwise the
/// embedded copy is materialised in a temp directory and that path is
/// returned.
pub fn onnxruntime_dylib() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("ORT_DYLIB_PATH") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
    }
    extract_embedded()
}

/// Write the embedded library to `<base>/pproni-<tag>/pproni.dll`,
/// reusing it if a correctly sized copy is already there.
///
/// `<base>` is the first of `%LOCALAPPDATA%`, the system temp dir, and the
/// executable's own directory that we can actually write into — so a machine
/// with an unset or read-only `TEMP` still starts.
fn extract_embedded() -> Result<PathBuf, String> {
    let subdir = format!("pproni-{}", cache_tag());
    let mut last_err = String::from("no writable directory for the ONNX Runtime");

    for base in candidate_bases() {
        let dir = base.join(&subdir);
        let dylib = dir.join(DYLIB_NAME);

        if fs::metadata(&dylib).map(|m| m.len()).ok() == Some(DYLIB_BYTES.len() as u64) {
            return Ok(dylib);
        }
        if let Err(e) = fs::create_dir_all(&dir).map_err(|e| e.to_string()) {
            last_err = format!("create {}: {e}", dir.display());
            continue;
        }
        match write_atomically(&dir, &dylib) {
            Ok(()) => return Ok(dylib),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

/// Directories to try as the extraction root, best first.
fn candidate_bases() -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        bases.push(PathBuf::from(p));
    }
    bases.push(std::env::temp_dir());
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf))
    {
        bases.push(dir);
    }
    bases
}

/// Write to a uniquely named sibling, then rename into place. A half-written
/// file never carries the final name, and concurrent CSP processes racing on
/// the same target are harmless — the loser sees the winner's finished file.
fn write_atomically(dir: &Path, dest: &Path) -> Result<(), String> {
    let tmp = dir.join(format!(
        "{}.{}.tmp",
        DYLIB_NAME,
        std::process::id()
    ));

    let mut f = fs::File::create(&tmp)
        .map_err(|e| format!("create {}: {e}", tmp.display()))?;
    f.write_all(DYLIB_BYTES)
        .and_then(|_| f.sync_all())
        .map_err(|e| format!("write {}: {e}", tmp.display()))?;
    drop(f);

    match fs::rename(&tmp, dest) {
        Ok(()) => Ok(()),
        Err(_) if fs::metadata(dest).map(|m| m.len()).ok()
            == Some(DYLIB_BYTES.len() as u64) =>
        {
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(format!("rename into {}: {e}", dest.display()))
        }
    }
}

/// A short hex tag identifying this exact runtime build: its length plus an
/// FNV-1a hash of the head, middle and tail. Cheap to compute, and distinct
/// whenever the embedded DLL changes.
fn cache_tag() -> String {
    const SAMPLE: usize = 4096;
    let n = DYLIB_BYTES.len();

    let mut h: u64 = 0xcbf29ce484222325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    feed(&(n as u64).to_le_bytes());
    feed(&DYLIB_BYTES[..SAMPLE.min(n)]);
    feed(&DYLIB_BYTES[n / 2..(n / 2 + SAMPLE).min(n)]);
    feed(&DYLIB_BYTES[n.saturating_sub(SAMPLE)..]);

    format!("{n:x}-{h:016x}")
}
