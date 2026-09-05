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
use crate::cmd::{count_done, paint_status, progress_bar, status_text};
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

    let mut milestones: Vec<Option<&crate::config::Milestone>> =
        cfg.milestones_ordered().into_iter().map(Some).collect();
    // A trailing pseudo-milestone for anything not scheduled yet.
    if items.iter().any(|i| i.milestone().is_none()) {
        milestones.push(None);
    }
    if let Some(want) = &args.milestone {
        milestones.retain(|m| m.is_some_and(|m| m.name == *want));
        if milestones.is_empty() {
            anyhow::bail!("unknown milestone `{want}`");
        }
    }

    println!("{}", style::bold(&cfg.project.name));
    if let Some(d) = &cfg.project.description {
        println!("{}", style::dim(d));
    }
    println!();

    for m in milestones {
        let members: Vec<&Item> = items
            .iter()
            .filter(|i| match m {
                Some(ms) => i.milestone() == Some(ms.name.as_str()),
                None => i.milestone().is_none(),
            })
            .collect();
        let done = count_done(&cfg, &members);

        let (name, title, due) = match m {
            Some(ms) => (ms.name.clone(), ms.title.clone(), ms.due.clone()),
            None => ("unscheduled".to_string(), None, None),
        };
        let mut heading = match &title {
            Some(t) => format!("{}  {}", style::bold(&name), style::dim(t)),
            None => style::bold(&name),
        };
        if let Some(ms) = m
            && let Some(state) = &ms.status
        {
            heading.push_str(&style::dim(&format!("  [{state}]")));
        }
        println!("{heading}");

        let bar = progress_bar(done, members.len(), 20);
        let mut meta = format!("  {bar}  {done}/{}", members.len());
        if let Some(d) = &due {
            meta.push_str(&style::dim(&format!("   due {d}")));
        }
        println!("{meta}");

        if let Some(ms) = m
            && let Some(desc) = &ms.description
        {
            println!("  {}", style::dim(desc));
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
