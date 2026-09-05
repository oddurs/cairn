// cairn — src/cmd/render_cmd.rs
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
        if current == markdown {
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

    let unchanged = std::fs::read_to_string(&target).is_ok_and(|c| c == markdown);
    crate::store::write_atomic(&target, markdown.as_bytes())?;
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
