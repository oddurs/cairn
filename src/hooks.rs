// cairn — hooks: the extension point.
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
//
// cairn does not embed a scripting language; it runs yours. A hook is a shell
// command named in cairn.toml, given the event as environment variables and the
// full item as JSON on stdin. Every hook fires *after* the change is durable,
// so a failing hook warns but never rolls anything back or fails the command —
// the same contract as git's post-* hooks.
use crate::config::{Config, Hook};
use crate::item::Item;
use crate::store::Store;
use crate::style;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

static DISABLED: AtomicBool = AtomicBool::new(false);
static SILENT: AtomicBool = AtomicBool::new(false);

/// Called for `--no-hooks`.
pub fn disable() {
    DISABLED.store(true, Ordering::Relaxed);
}

/// Discard hook stdout. Used by the MCP server, whose stdout carries protocol
/// framing that anything a hook prints would corrupt.
pub fn silence_output() {
    SILENT.store(true, Ordering::Relaxed);
}

fn enabled() -> bool {
    !DISABLED.load(Ordering::Relaxed) && std::env::var_os("CAIRN_NO_HOOKS").is_none()
}

// The shared `After` prefix is deliberate: these names mirror the cairn.toml
// keys exactly, and every hook is by design an after-the-fact notification.
#[allow(clippy::enum_variant_names)]
#[derive(Copy, Clone)]
pub enum Event {
    AfterCreate,
    AfterChange,
    AfterRemove,
    AfterRender,
}

impl Event {
    fn name(self) -> &'static str {
        match self {
            Event::AfterCreate => "after-create",
            Event::AfterChange => "after-change",
            Event::AfterRemove => "after-remove",
            Event::AfterRender => "after-render",
        }
    }

    fn hook(self, cfg: &Config) -> Option<&Hook> {
        match self {
            Event::AfterCreate => cfg.hooks.after_create.as_ref(),
            Event::AfterChange => cfg.hooks.after_change.as_ref(),
            Event::AfterRemove => cfg.hooks.after_remove.as_ref(),
            Event::AfterRender => cfg.hooks.after_render.as_ref(),
        }
    }
}

/// Fire an item-scoped hook. Errors are reported, never propagated.
pub fn item(cfg: &Config, store: &Store, event: Event, it: &Item) {
    let Some(hook) = event.hook(cfg) else {
        return;
    };
    if !enabled() {
        return;
    }
    let json = crate::cmd::item_json(cfg, it, store, true);
    let env = vec![
        ("CAIRN_ITEM_ID", cfg.format_id(it.id)),
        ("CAIRN_ITEM_PATH", store.rel(&it.path)),
        ("CAIRN_ITEM_TITLE", it.title().to_string()),
        ("CAIRN_ITEM_STATUS", it.status().to_string()),
        ("CAIRN_ITEM_TYPE", it.kind().unwrap_or("").to_string()),
        (
            "CAIRN_ITEM_MILESTONE",
            it.milestone().unwrap_or("").to_string(),
        ),
        (
            "CAIRN_ITEM_CATEGORY",
            cfg.category(it.status()).as_str().to_string(),
        ),
    ];
    spawn(cfg, event, hook, env, &json.to_string());
}

/// Fire the render hook, which is about a file rather than an item.
pub fn render(cfg: &Config, target: &str, count: usize) {
    let Some(hook) = Event::AfterRender.hook(cfg) else {
        return;
    };
    if !enabled() {
        return;
    }
    let env = vec![
        ("CAIRN_RENDER_TARGET", target.to_string()),
        ("CAIRN_ITEM_COUNT", count.to_string()),
    ];
    let payload = serde_json::json!({ "target": target, "items": count });
    spawn(cfg, Event::AfterRender, hook, env, &payload.to_string());
}

fn spawn(cfg: &Config, event: Event, hook: &Hook, env: Vec<(&str, String)>, stdin: &str) {
    let mut cmd = match hook {
        Hook::Argv(argv) => {
            let Some((program, rest)) = argv.split_first() else {
                return warn(event, "is an empty command");
            };
            let mut c = Command::new(program);
            c.args(rest);
            c
        }
        Hook::Shell(script) => shell_command(script),
    };
    cmd.current_dir(&cfg.root)
        .env("CAIRN_EVENT", event.name())
        .env("CAIRN_ROOT", &cfg.root)
        .env("CAIRN_CONFIG", cfg.root.join(crate::config::CONFIG_FILE))
        // Guard against a hook that calls cairn and re-triggers itself.
        .env("CAIRN_NO_HOOKS", "1")
        .stdin(Stdio::piped());
    if SILENT.load(Ordering::Relaxed) {
        cmd.stdout(Stdio::null());
    }
    for (k, v) in env {
        cmd.env(k, v);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return warn(event, &format!("could not run: {e}")),
    };
    if let Some(mut pipe) = child.stdin.take() {
        // A hook that ignores stdin closes the pipe early; that is not an error.
        let _ = pipe.write_all(stdin.as_bytes());
    }
    match child.wait() {
        Ok(status) if status.success() => {}
        Ok(status) => warn(event, &format!("exited with {status}")),
        Err(e) => warn(event, &format!("failed: {e}")),
    }
}

/// Build the platform's shell invocation.
///
/// On Windows this deliberately bypasses `Command::arg`. Rust quotes arguments
/// for the MSVC C runtime convention, which `cmd.exe` does not follow, so a
/// command containing quotes or `&` arrives mangled. `raw_arg` hands the string
/// to `cmd.exe` exactly as written, which is the only correct way to pass it.
#[cfg(windows)]
fn shell_command(script: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new("cmd");
    c.raw_arg("/C ").raw_arg(script);
    c
}

#[cfg(not(windows))]
fn shell_command(script: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(script);
    c
}

fn warn(event: Event, msg: &str) {
    eprintln!(
        "{} hook `{}` {msg}",
        style::yellow("warning:"),
        event.name()
    );
}
