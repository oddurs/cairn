// cairn — migrating a project between on-disk formats.
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
// There is nothing to migrate yet: format 1 is the only format there has ever
// been. The command exists anyway, and is tested, because a migration path
// invented at the moment it is first needed is a migration path nobody has
// tried. This one is exercised on every run, so when a format 2 arrives the
// scaffolding around it is already known to work.
use crate::config::{CONFIG_FILE, CURRENT_FORMAT, Config};
use crate::lock::Lock;
use crate::store::Store;
use crate::style;
use anyhow::Result;
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Report what would change and write nothing
    #[arg(short = 'n', long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Exit non-zero if the project is not already at the current format
    #[arg(long, action = ArgAction::SetTrue)]
    pub check: bool,

    /// Print nothing when there is nothing to do
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let from = cfg.format();

    if from == CURRENT_FORMAT {
        if args.check {
            if !args.quiet {
                println!("{} format {from}, which is current", style::green("ok:"));
            }
            return Ok(0);
        }
        if !args.quiet {
            println!(
                "{} already at format {CURRENT_FORMAT}; nothing to migrate",
                style::green("ok:")
            );
        }
        return Ok(0);
    }

    // Unreachable while 1 is the only format: `Config::load` refuses anything
    // higher, and there is nothing lower. Kept as the shape a real migration
    // will take.
    if args.check {
        eprintln!(
            "{} project is format {from}, current is {CURRENT_FORMAT} — run `cairn migrate`",
            style::red("stale:")
        );
        return Ok(1);
    }

    let steps = plan(from, CURRENT_FORMAT);
    for (from, to) in &steps {
        println!("  format {from} -> {to}");
    }
    if args.dry_run {
        println!(
            "{} {} step(s) would run",
            style::dim("dry run:"),
            steps.len()
        );
        return Ok(0);
    }

    let _lock = Lock::acquire(&cfg)?;
    let store = Store::new(&cfg);
    // A migration must never run against a backlog it cannot fully read.
    let items = store.load_all()?;
    for (from, to) in &steps {
        apply(&cfg, &items, *from, *to)?;
    }
    stamp(&cfg, CURRENT_FORMAT)?;

    println!(
        "{} {} item(s) now at format {CURRENT_FORMAT}",
        style::green("migrated:"),
        items.len()
    );
    Ok(0)
}

/// The chain of single-step migrations between two formats.
fn plan(from: u32, to: u32) -> Vec<(u32, u32)> {
    (from..to).map(|n| (n, n + 1)).collect()
}

fn apply(_cfg: &Config, _items: &[crate::item::Item], from: u32, to: u32) -> Result<()> {
    anyhow::bail!("no migration is defined from format {from} to {to}")
}

/// Record the new format in cairn.toml, preserving its comments.
fn stamp(cfg: &Config, format: u32) -> Result<()> {
    let path = cfg.root.join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path)?;
    let mut doc: toml_edit::DocumentMut = text.parse()?;
    doc["format"] = toml_edit::value(format as i64);
    crate::store::write_atomic(&path, doc.to_string().as_bytes())
}
