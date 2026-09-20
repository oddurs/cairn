// cairn — src/cmd/render_cmd.rs
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
// cairn render — write the generated roadmap file.
use crate::config::Config;
use crate::render::roadmap_markdown;
use crate::store::Store;
use crate::{hooks, style};
use anyhow::Result;
use clap::ArgAction;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    /// Write somewhere other than render.target ("-" for stdout)
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,

    /// Exit non-zero if the file on disk is out of date; write nothing
    #[arg(long, action = ArgAction::SetTrue)]
    pub check: bool,

    /// Print nothing on success
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_all()?;
    // Said here rather than inside `roadmap_markdown`, which the after-change
    // hook calls after every write: a true warning repeated on every command is
    // how people learn to skim the line a real one appears on. `-q` is what the
    // hook passes, so a person running `cairn render` hears it and the hook
    // stays silent. `cairn check` reports it with the line number either way.
    if !args.quiet
        && let Some(expr) = &cfg.render.include
        && let Ok(f) = crate::filter::Filter::parse(expr)
    {
        crate::filter::warn_unknown_keys(&cfg, "render.include", &f);
    }
    let markdown = roadmap_markdown(&cfg, &store, &items)?;

    if args.output.as_deref() == Some("-") {
        print!("{markdown}");
        return Ok(0);
    }

    let target: PathBuf = match &args.output {
        Some(o) => cfg.root.join(o),
        None => cfg.root.join(&cfg.render.target),
    };

    if args.check {
        let current = std::fs::read_to_string(&target).unwrap_or_default();
        if crate::render::matches_on_disk(&current, &markdown) {
            if !args.quiet {
                println!("{} {}", style::green("up to date"), store.rel(&target));
            }
            return Ok(0);
        }
        eprintln!(
            "{} {} is out of date — run `cairn render`",
            style::red("stale"),
            store.rel(&target)
        );
        return Ok(1);
    }

    let current = std::fs::read_to_string(&target).ok();
    let unchanged = current
        .as_deref()
        .is_some_and(|c| crate::render::matches_on_disk(c, &markdown));
    // Written with the ending the file already had, so rendering a CRLF checkout
    // does not convert it and turn a no-op into a diff of every line.
    let out = crate::render::as_written(current.as_deref(), &markdown);
    crate::store::write_atomic(&target, out.as_bytes())?;
    hooks::render(&cfg, &store.rel(&target), items.len());
    if !args.quiet {
        let verb = if unchanged { "unchanged" } else { "wrote" };
        println!(
            "{} {}  {}",
            style::green(verb),
            store.rel(&target),
            style::dim(&format!("{} items", items.len()))
        );
    }
    Ok(0)
}
