// cairn — repairing item ids.
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
// Sequential ids need a counter, and a counter needs coordination that a
// distributed workflow cannot provide: two branches each create "the next"
// item and both pick the same number. Git merges them cleanly — the filenames
// differ — and you are left with two items sharing an id.
//
// This command is the repair. It is deliberately not automatic: renumbering
// rewrites files, so it happens when you ask for it, not behind your back.
use crate::config::Config;
use crate::item::Item;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::style;
use anyhow::{Context, Result, bail};
use clap::ArgAction;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    /// Renumber every item to a gapless 1..n sequence
    #[arg(long, action = ArgAction::SetTrue)]
    pub compact: bool,

    /// Show what would change without touching anything
    #[arg(short = 'n', long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Print nothing when there is nothing to do
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let _lock = Lock::acquire(&cfg)?;
    let mut items = store.load_all()?;
    // Ordering decides which file keeps a contested id, so it is chosen rather
    // than incidental: oldest first, because the item that existed before the
    // collision should keep its number and the branch that arrived later should
    // move. Items with no creation date sort last, and the path breaks
    // remaining ties so two people running this on the same tree agree.
    items.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then_with(|| created_key(a).cmp(&created_key(b)))
            .then_with(|| a.path.cmp(&b.path))
    });

    let duplicates = duplicate_ids(&items);
    let plan = if args.compact {
        if !duplicates.is_empty() {
            bail!(
                "cannot compact while {} id(s) are duplicated — run `cairn renumber` first",
                duplicates.len()
            );
        }
        compact_plan(&items)
    } else {
        if duplicates.is_empty() {
            if !args.quiet {
                println!("{} no duplicate ids", style::green("ok:"));
            }
            return Ok(0);
        }
        duplicate_plan(&items, &duplicates)
    };

    if plan.is_empty() {
        if !args.quiet {
            println!("{} nothing to renumber", style::green("ok:"));
        }
        return Ok(0);
    }

    for (index, new_id) in &plan {
        let it = &items[*index];
        println!(
            "  {} {} {}  {}",
            style::dim(&cfg.format_id(it.id)),
            style::dim("->"),
            style::bold(&cfg.format_id(*new_id)),
            it.title()
        );
    }

    if args.dry_run {
        println!(
            "\n{} {} item(s) would be renumbered",
            style::dim("dry run:"),
            plan.len()
        );
        return Ok(0);
    }

    // Under --compact every id moves, so references must move with them. When
    // repairing duplicates the retained item keeps its id, and nothing can
    // unambiguously refer to the copy, so references are left alone.
    let id_map: HashMap<u32, u32> = if args.compact {
        plan.iter().map(|(i, new)| (items[*i].id, *new)).collect()
    } else {
        HashMap::new()
    };

    apply(&cfg, &store, &mut items, &plan, &id_map)?;

    println!("{} {} item(s)", style::green("renumbered:"), plan.len());
    if !args.compact {
        eprintln!(
            "{} existing `depends_on` references still point at the retained items; \
             check whether any should point at the renumbered ones",
            style::yellow("note:")
        );
    }
    Ok(0)
}

/// Sort key for creation date: undated items sort after dated ones.
fn created_key(item: &Item) -> (bool, String) {
    match item.meta.created.as_deref() {
        Some(d) if !d.is_empty() => (false, d.to_string()),
        _ => (true, String::new()),
    }
}

/// Ids held by more than one file, with the indices of every item holding them.
fn duplicate_ids(items: &[Item]) -> BTreeMap<u32, Vec<usize>> {
    let mut by_id: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        by_id.entry(it.id).or_default().push(i);
    }
    by_id.retain(|_, v| v.len() > 1);
    by_id
}

/// The oldest item holding a duplicated id keeps it; the rest get fresh ones.
fn duplicate_plan(items: &[Item], duplicates: &BTreeMap<u32, Vec<usize>>) -> Vec<(usize, u32)> {
    let mut next = items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
    let mut plan = Vec::new();
    for indices in duplicates.values() {
        for index in indices.iter().skip(1) {
            plan.push((*index, next));
            next += 1;
        }
    }
    plan
}

fn compact_plan(items: &[Item]) -> Vec<(usize, u32)> {
    items
        .iter()
        .enumerate()
        .map(|(i, _)| (i, i as u32 + 1))
        .filter(|(i, new)| items[*i].id != *new)
        .collect()
}

/// Two phases, so a rename can never land on a file that has not moved yet.
fn apply(
    cfg: &Config,
    store: &Store,
    items: &mut [Item],
    plan: &[(usize, u32)],
    id_map: &HashMap<u32, u32>,
) -> Result<()> {
    let mut staged: Vec<(usize, PathBuf)> = Vec::new();
    for (index, _) in plan {
        let from = items[*index].path.clone();
        let temp = from.with_extension("md.renumber");
        std::fs::rename(&from, &temp).with_context(|| format!("staging {}", from.display()))?;
        staged.push((*index, temp));
    }

    for ((index, new_id), (_, temp)) in plan.iter().zip(staged.iter()) {
        let it = &mut items[*index];
        it.id = *new_id;
        it.meta.id = Some(*new_id);
        it.path = store.path_for(*new_id, it.title());
        it.touch(&today());
        it.save()?;
        std::fs::remove_file(temp).with_context(|| format!("removing {}", temp.display()))?;
    }

    if !id_map.is_empty() {
        for it in items.iter_mut() {
            let before = it.meta.depends_on.clone();
            for dep in it.meta.depends_on.iter_mut() {
                if let Some(new) = id_map.get(dep) {
                    *dep = *new;
                }
            }
            it.meta.depends_on.sort_unstable();
            if it.meta.depends_on != before {
                it.touch(&today());
                it.save()?;
                println!(
                    "  {} references updated in {}",
                    style::dim(&cfg.format_id(it.id)),
                    store.rel(&it.path)
                );
            }
        }
    }
    Ok(())
}
