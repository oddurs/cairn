// cairn — full-text search across items.
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
use crate::cmd::{item_json, paint_status};
use crate::config::Config;
use crate::filter::{Ctx, Filter, sort_items};
use crate::item::Item;
use crate::store::Store;
use crate::style;
use crate::table::clip;
use anyhow::Result;
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Text to look for, case-insensitively
    #[arg(value_name = "QUERY")]
    pub query: String,

    /// Search titles only
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub titles: bool,

    /// Additional filter expression
    #[arg(short, long, value_name = "EXPR")]
    pub filter: Option<String>,

    /// Include done and dropped items
    #[arg(short = 'A', long, action = ArgAction::SetTrue)]
    pub all: bool,

    /// Show at most N items
    #[arg(short = 'n', long, value_name = "N")]
    pub limit: Option<usize>,

    /// Output JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,

    /// Output ids only
    #[arg(long, action = ArgAction::SetTrue)]
    pub ids: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_for_reading()?;
    let ctx = Ctx::new(&cfg, &items);

    let filter = match &args.filter {
        Some(expr) => Filter::parse(expr)?,
        None => Filter::default(),
    };
    let needle = args.query.to_lowercase();

    let mut hits: Vec<Item> = items
        .iter()
        .filter(|i| args.all || !ctx.is_closed(i))
        .filter(|i| filter.matches(i, &ctx))
        .filter(|i| matches(i, &needle, args.titles))
        .cloned()
        .collect();
    sort_items(&mut hits, "milestone,status,id", &ctx);
    if let Some(n) = args.limit {
        hits.truncate(n);
    }

    if args.ids {
        for i in &hits {
            println!("{}", cfg.format_id(i.id));
        }
        return Ok(0);
    }
    if args.json {
        let arr: Vec<_> = hits
            .iter()
            .map(|i| {
                let mut v = item_json(&cfg, i, &store, false);
                if let Some(o) = v.as_object_mut()
                    && let Some(line) = context_line(i, &needle)
                {
                    o.insert("match".into(), serde_json::json!(line));
                }
                v
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(0);
    }

    if hits.is_empty() {
        eprintln!(
            "{}",
            style::dim(&format!("no items match `{}`", args.query))
        );
        return Ok(1);
    }
    for i in &hits {
        println!(
            "{}  {}  {}",
            style::dim(&cfg.format_id(i.id)),
            paint_status(&cfg, i.status()),
            style::bold(i.title())
        );
        // One line of body context, the way grep would show it.
        if let Some(line) = context_line(i, &needle)
            && !args.titles
        {
            println!("      {}", style::dim(&clip(&line, 100)));
        }
    }
    Ok(0)
}

fn matches(item: &Item, needle: &str, titles_only: bool) -> bool {
    if item.title().to_lowercase().contains(needle) {
        return true;
    }
    if titles_only {
        return false;
    }
    item.meta
        .labels
        .iter()
        .any(|l| l.to_lowercase().contains(needle))
        || item.body.to_lowercase().contains(needle)
}

fn context_line(item: &Item, needle: &str) -> Option<String> {
    item.body
        .lines()
        .map(str::trim)
        .find(|l| l.to_lowercase().contains(needle))
        .map(str::to_string)
}
