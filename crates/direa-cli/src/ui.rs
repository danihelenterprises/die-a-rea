//! Minimal verbosity-aware output for the CLI (no logging dependency).
//!
//! `--verbose` (repeatable) enables diagnostics on stderr; `--quiet` suppresses
//! primary output. Level 1 = phases/timing, level 2 = transform trace.

use std::fmt::Display;
use std::time::Instant;

pub struct Ui {
    pub verbosity: u8,
    pub quiet: bool,
}

impl Ui {
    pub fn new(verbosity: u8, quiet: bool) -> Self {
        Ui { verbosity, quiet }
    }

    /// Diagnostic line to stderr, gated by verbosity.
    pub fn info(&self, level: u8, msg: impl Display) {
        if !self.quiet && self.verbosity >= level {
            eprintln!("[die-a-rea] {msg}");
        }
    }

    /// Primary result line to stdout, suppressed by `--quiet`.
    pub fn out(&self, msg: impl Display) {
        if !self.quiet {
            println!("{msg}");
        }
    }

    /// Run `f`, reporting its elapsed time at `level`.
    pub fn timed<T>(&self, level: u8, label: &str, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let value = f();
        self.info(level, format_args!("{label}: {:?}", start.elapsed()));
        value
    }
}
