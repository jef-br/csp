//! Throttled, in-place batch progress bar with a fixed footer under it.
//!
//! `batch::run` fans work across cores with rayon and calls [`Progress::tick`] once per finished
//! file from whatever worker thread finished it. Redraws are throttled to ~15 Hz through a
//! non-blocking `try_lock`: a worker that can't take the gate just bumps the counter and returns,
//! so the bar never sits on the batch's critical path. The bar disables itself when stdout is not
//! a terminal, which keeps it out of the piped `csp <in> <out>` dev path.
//!
//! The footer is the block the shell wants held under the bar — the rule / callout / rule that
//! tells the user how to interrupt. Bar and footer are repainted together as one unit: each frame
//! walks the cursor back to the top of the block, erases to the end of the screen, and reprints.
//! Nothing else writes to stdout while the batch runs (failures are collected, not printed), so
//! the block stays where it was first drawn.

use super::theme::{ORANGE, ORANGE_DIM, RESET};
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WIDTH: usize = 40;
const FULL: char = '\u{2588}'; // █  done
const EMPTY: char = '\u{2591}'; // ░  remaining
const MIN_REDRAW: Duration = Duration::from_millis(66); // ~15 Hz
/// Move to the top of the block and clear everything below it.
const ERASE_DOWN: &str = "\x1b[0J";

pub struct Progress {
    total: usize,
    done: AtomicUsize,
    start: Instant,
    /// Guards stdout and holds the last render time. `try_lock` is the throttle.
    gate: Mutex<Instant>,
    enabled: bool,
    /// Lines held under the bar, repainted with it.
    footer: Vec<String>,
}

impl Progress {
    /// Draw the first frame — bar plus `footer` — and hand back a handle the workers tick. Pass an
    /// empty `footer` for a bar with nothing beneath it.
    pub fn new(total: usize, footer: &[String]) -> Progress {
        let progress = Progress {
            total,
            done: AtomicUsize::new(0),
            start: Instant::now(),
            // Start in the past so the first tick draws immediately.
            gate: Mutex::new(Instant::now() - MIN_REDRAW),
            enabled: total > 0 && std::io::stdout().is_terminal(),
            footer: footer.to_vec(),
        };
        if progress.enabled {
            let mut out = std::io::stdout().lock();
            progress.paint(&mut out, 0, false);
        }
        progress
    }

    /// Record one finished file (success or failure) and redraw if the throttle allows.
    pub fn tick(&self) {
        if !self.enabled {
            return;
        }
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;

        if done >= self.total {
            // Final frame always draws, throttle or not.
            if let Ok(mut last) = self.gate.lock() {
                self.redraw(self.total);
                *last = Instant::now();
            }
            return;
        }

        if let Ok(mut last) = self.gate.try_lock() {
            if last.elapsed() >= MIN_REDRAW {
                self.redraw(done);
                *last = Instant::now();
            }
        }
    }

    fn redraw(&self, done: usize) {
        let mut out = std::io::stdout().lock();
        self.paint(&mut out, done, true);
    }

    /// Print the block. When `rewind`, first walk back over the frame already on screen and wipe
    /// it, so the new frame lands in exactly the same place.
    fn paint<W: Write>(&self, out: &mut W, done: usize, rewind: bool) {
        if rewind {
            let up = self.footer.len() + 1;
            let _ = write!(out, "\x1b[{up}A\r{ERASE_DOWN}");
        }
        let _ = writeln!(out, "{}{}", self.bar(done), self.label(done));
        for line in &self.footer {
            let _ = writeln!(out, "{line}");
        }
        let _ = out.flush();
    }

    /// `40%, 01:12 remaining` — empty on the closing frame, where nothing is left to wait for.
    fn label(&self, done: usize) -> String {
        if done == 0 || done >= self.total {
            return String::new();
        }
        let per = self.start.elapsed().as_secs_f64() / done as f64;
        let rem = (per * (self.total - done) as f64).round() as u64;
        let pct = (done as f64 / self.total as f64 * 100.0) as u32;
        format!(" {pct}%, {:02}:{:02} remaining", rem / 60, rem % 60)
    }

    /// The bar itself: filled cells in solid orange, the remainder in a dim orange shade.
    fn bar(&self, done: usize) -> String {
        let frac = done as f64 / self.total.max(1) as f64;
        let filled = ((frac * WIDTH as f64).round() as usize).min(WIDTH);
        format!(
            "{ORANGE}{}{ORANGE_DIM}{}{RESET}",
            String::from(FULL).repeat(filled),
            String::from(EMPTY).repeat(WIDTH - filled),
        )
    }
}
