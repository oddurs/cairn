// cairn — src/cmd/roadmap.rs
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
// cairn roadmap — the milestone view, in the terminal.
use crate::cmd::{paint_status, progress, progress_bar, status_text};
use crate::config::Config;
use crate::item::Item;
use crate::store::Store;
use crate::style;
use anyhow::Result;
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Show only this milestone
    #[arg(value_name = "MILESTONE")]
    pub milestone: Option<String>,

    /// List the items under each milestone
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub items: bool,

    /// Include done and dropped items in the listing
    #[arg(short = 'A', long, action = ArgAction::SetTrue)]
    pub all: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_for_reading()?;

    let ctx = crate::filter::Ctx::new(&cfg, &items);
    // The roadmap is about work. A container is what work is scheduled
    // against, so counting one as unscheduled work would put every milestone
    // in a heap at the bottom of its own roadmap.
    let work: Vec<Item> = items
        .iter()
        .filter(|i| !cfg.is_container(i.kind()))
        .cloned()
        .collect();
    let mut milestones: Vec<Option<&Item>> = ctx.milestones.iter().map(|m| Some(*m)).collect();
    // A trailing pseudo-milestone for anything not scheduled yet.
    if work.iter().any(|i| i.milestone().is_none()) {
        milestones.push(None);
    }
    if let Some(want) = &args.milestone {
        milestones
            .retain(|m| m.is_some_and(|m| m.key().is_some_and(|k| k.eq_ignore_ascii_case(want))));
        if milestones.is_empty() {
            anyhow::bail!(
                "unknown milestone `{want}`\nknown: {}",
                ctx.milestones.keys().join(", ")
            );
        }
    }

    println!("{}", style::bold(&cfg.project.name));
    if let Some(d) = &cfg.project.description {
        println!("{}", style::dim(d));
    }
    println!();

    for m in milestones {
        let members: Vec<&Item> = work
            .iter()
            .filter(|i| match m {
                Some(ms) => i
                    .milestone()
                    .is_some_and(|v| ms.key().is_some_and(|k| k.eq_ignore_ascii_case(v))),
                None => i.milestone().is_none(),
            })
            .collect();
        let (done, total) = progress(&cfg, &members);

        let (name, title, due) = match m {
            Some(ms) => (
                ms.key().unwrap_or_default().to_string(),
                Some(ms.title().to_string()),
                crate::refs::due(ms).map(str::to_string),
            ),
            None => ("unscheduled".to_string(), None, None),
        };
        let mut heading = match &title {
            Some(t) => format!("{}  {}", style::bold(&name), style::dim(t)),
            None => style::bold(&name),
        };
        // A milestone has a status of its own now, because it is an item. Shown
        // only when it says something: every milestone sitting at the initial
        // status would be a column of noise.
        if let Some(ms) = m
            && ms.status() != cfg.initial_status()
        {
            heading.push_str(&style::dim(&format!("  [{}]", ms.status())));
        }
        println!("{heading}");

        let bar = progress_bar(done, total, 20);
        let mut meta = format!("  {bar}  {done}/{total}");
        if let Some(d) = &due {
            meta.push_str(&style::dim(&format!("   due {d}")));
        }
        println!("{meta}");

        // The body is the description, which is most of why a milestone is an
        // item: the reason for a date lives with the date.
        if let Some(ms) = m {
            let summary = ms.summary();
            if !summary.trim().is_empty() {
                println!("  {}", style::dim(&summary));
            }
        }

        if args.items {
            for i in &members {
                if !args.all && cfg.category(i.status()).is_closed() {
                    continue;
                }
                // Pad on the visible label, not the escape-coded one.
                let label = status_text(&cfg, i.status());
                let pad = " ".repeat(12usize.saturating_sub(label.chars().count()));
                println!(
                    "    {}  {}{pad}  {}",
                    style::dim(&cfg.format_id(i.id)),
                    paint_status(&cfg, i.status()),
                    i.title()
                );
            }
        }
        println!();
    }
    Ok(0)
}
