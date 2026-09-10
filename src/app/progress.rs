//! Throttled, in-place batch progress bar.
//!
//! `batch::run` fans work across cores with rayon and calls [`Progress::tick`] once per finished
//! file from whatever worker thread finished it. Redraws are throttled to ~15 Hz through a
//! non-blocking `try_lock`: a worker that can't take the gate just bumps the counter and returns,
//! so the bar never sits on the batch's critical path. The bar disables itself when stdout is not
//! a terminal, which keeps it out of the piped `csp <in> <out>` dev path.

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WIDTH: usize = 40;
const FULL: char = '\u{2588}'; // █  done
const EMPTY: char = '\u{2591}'; // ░  remaining
const MIN_REDRAW: Duration = Duration::from_millis(66); // ~15 Hz
const CLEAR_LINE: &str = "\r\x1b[2K";

pub struct Progress {
    total: usize,
    done: AtomicUsize,
    start: Instant,
    /// Guards stdout and holds the last render time. `try_lock` is the throttle.
    gate: Mutex<Instant>,
    enabled: bool,
}

impl Progress {
    pub fn new(total: usize) -> Progress {
        Progress {
            total,
            done: AtomicUsize::new(0),
            start: Instant::now(),
            // Start in the past so the first tick draws immediately.
            gate: Mutex::new(Instant::now() - MIN_REDRAW),
            enabled: total > 0 && std::io::stdout().is_terminal(),
        }
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
                self.finish_line();
                *last = Instant::now();
            }
            return;
        }

        if let Ok(mut last) = self.gate.try_lock() {
            if last.elapsed() >= MIN_REDRAW {
                self.render(done);
                *last = Instant::now();
            }
        }
    }

    /// Print a failure line above the bar without smearing it. Takes the gate with a blocking
    /// `lock` so the message is never dropped.
    pub fn note_failure(&self, msg: &str) {
        if !self.enabled {
            eprintln!("{msg}");
            return;
        }
        if let Ok(mut last) = self.gate.lock() {
            let mut out = std::io::stdout().lock();
            let _ = writeln!(out, "{CLEAR_LINE}{msg}");
            let _ = out.flush();
            let done = self.done.load(Ordering::Relaxed).min(self.total);
            if done < self.total {
                self.render_to(&mut out, done);
            }
            *last = Instant::now();
        }
    }

    fn render(&self, done: usize) {
        let mut out = std::io::stdout().lock();
        self.render_to(&mut out, done);
    }

    fn render_to<W: Write>(&self, out: &mut W, done: usize) {
        let frac = done as f64 / self.total as f64;
        let filled = ((frac * WIDTH as f64).round() as usize).min(WIDTH);
        let bar = format!(
            "{}{}",
            String::from(FULL).repeat(filled),
            String::from(EMPTY).repeat(WIDTH - filled),
        );

        let per = self.start.elapsed().as_secs_f64() / done as f64;
        let rem = (per * (self.total - done) as f64).round() as u64;
        let (mm, ss) = (rem / 60, rem % 60);
        let pct = (frac * 100.0) as u32;

        let _ = write!(out, "{CLEAR_LINE}{bar} {pct}%, {mm:02}:{ss:02} remaining");
        let _ = out.flush();
    }

    /// 100% frame: full bar, no label (the "remaining" field disappears), then a newline so the
    /// following `println!` starts on a clean line.
    fn finish_line(&self) {
        let bar = String::from(FULL).repeat(WIDTH);
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{CLEAR_LINE}{bar}");
        let _ = out.flush();
    }
}
