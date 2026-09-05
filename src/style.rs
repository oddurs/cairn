// cairn — terminal styling. Deliberately dependency-free: a handful of SGR
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.
use std::io::IsTerminal;
use std::sync::atomic::{AtomicBool, Ordering};

static COLOR: AtomicBool = AtomicBool::new(false);

/// `force` mirrors the tri-state `--color` flag: Some(true)/Some(false) override,
/// None means auto-detect (NO_COLOR, then tty).
pub fn init(force: Option<bool>) {
    let on = match force {
        Some(v) => v,
        None => std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal(),
    };
    COLOR.store(on, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    COLOR.load(Ordering::Relaxed)
}

pub fn paint(code: &str, s: &str) -> String {
    if enabled() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

pub fn bold(s: &str) -> String {
    paint("1", s)
}
pub fn dim(s: &str) -> String {
    paint("2", s)
}
pub fn red(s: &str) -> String {
    paint("31", s)
}
pub fn green(s: &str) -> String {
    paint("32", s)
}
pub fn yellow(s: &str) -> String {
    paint("33", s)
}

/// Resolve a colour name from `cairn.toml` to an SGR code.
pub fn named(name: &str, s: &str) -> String {
    let code = match name.trim().to_ascii_lowercase().as_str() {
        "black" => "30",
        "red" => "31",
        "green" => "32",
        "yellow" => "33",
        "blue" => "34",
        "magenta" | "purple" => "35",
        "cyan" => "36",
        "white" => "37",
        "gray" | "grey" | "dim" => "90",
        "bold" => "1",
        _ => return s.to_string(),
    };
    paint(code, s)
}
