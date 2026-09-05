// cairn — src/cmd/board.rs
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
// cairn board — a kanban board printed to stdout.
//
// Deliberately not a TUI: it is pipeable, screenshot-friendly and works over
// ssh in a dumb terminal.
use crate::cmd::paint_status;
use crate::config::Config;
use crate::filter::{Ctx, Filter, resolve, sort_items};
use crate::item::Item;
use crate::store::Store;
use crate::style;
use crate::table::clip;
use anyhow::{Result, bail};
use clap::ArgAction;
use terminal_size::{Width, terminal_size};
use unicode_width::UnicodeWidthStr;

#[derive(clap::Args)]
pub struct Args {
    /// Field to use for columns
    #[arg(short, long, value_name = "FIELD", default_value = "status")]
    pub group_by: String,

    /// Use a saved view from cairn.toml
    #[arg(long, value_name = "NAME")]
    pub view: Option<String>,

    /// Raw filter expression
    #[arg(short, long, value_name = "EXPR")]
    pub filter: Option<String>,

    /// Only this milestone
    #[arg(short, long, value_name = "MILESTONE")]
    pub milestone: Option<String>,

    /// Include done and dropped items
    #[arg(short = 'A', long, action = ArgAction::SetTrue)]
    pub all: bool,

    /// Column width
    #[arg(long, value_name = "N")]
    pub width: Option<usize>,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let mut items = store.load_for_reading()?;

    let mut group_by = args.group_by.clone();
    let mut filter = Filter::default();
    if let Some(name) = &args.view {
        let Some(v) = cfg.view(name) else {
            bail!("unknown view `{name}`");
        };
        if let Some(expr) = &v.filter {
            filter = filter.and(Filter::parse(expr)?);
        }
        if let Some(g) = &v.group_by
            && args.group_by == "status"
        {
            group_by = g.clone();
        }
    }
    if let Some(expr) = &args.filter {
        filter = filter.and(Filter::parse(expr)?);
    }
    if let Some(m) = &args.milestone {
        filter.push("milestone", crate::filter::Op::Eq, vec![m.clone()]);
    }
    let ctx = Ctx::new(&cfg, &items);
    items.retain(|i| filter.matches(i, &ctx));
    sort_items(&mut items, "milestone,id", &ctx);

    let columns = column_values(&ctx, &items, &group_by, args.all);
    if columns.is_empty() {
        eprintln!("{}", style::dim("nothing to show"));
        return Ok(0);
    }

    let term = terminal_size()
        .map(|(Width(w), _)| w as usize)
        .unwrap_or(100);
    let gap = 2;
    let n = columns.len();
    let width = args
        .width
        .unwrap_or_else(|| ((term.saturating_sub(gap * (n - 1))) / n).clamp(16, 40));

    let mut cells: Vec<Vec<String>> = Vec::with_capacity(n);
    let mut headers: Vec<String> = Vec::with_capacity(n);
    for value in &columns {
        let members: Vec<&Item> = items
            .iter()
            .filter(|i| matches_group(i, &ctx, &group_by, value))
            .collect();
        let title = if group_by == "status" {
            paint_status(&cfg, value)
        } else if value.is_empty() {
            style::dim("(none)")
        } else {
            value.clone()
        };
        headers.push(format!(
            "{title} {}",
            style::dim(&format!("{}", members.len()))
        ));
        cells.push(
            members
                .iter()
                .map(|i| {
                    format!(
                        "{} {}",
                        style::dim(&cfg.format_id(i.id)),
                        clip(i.title(), width.saturating_sub(cfg.project.id_width + 1))
                    )
                })
                .collect(),
        );
    }

    let sep = " ".repeat(gap);
    println!("{}", join_padded(&headers, width, &sep));
    println!(
        "{}",
        style::dim(&join_padded(&vec!["─".repeat(width); n], width, &sep))
    );

    let depth = cells.iter().map(Vec::len).max().unwrap_or(0);
    for row in 0..depth {
        let line: Vec<String> = cells
            .iter()
            .map(|c| c.get(row).cloned().unwrap_or_default())
            .collect();
        println!("{}", join_padded(&line, width, &sep));
    }
    Ok(0)
}

/// Pad on visible width, ignoring escape codes.
fn join_padded(cells: &[String], width: usize, sep: &str) -> String {
    let padded: Vec<String> = cells
        .iter()
        .map(|c| {
            let visible = visible_width(c);
            format!("{c}{}", " ".repeat(width.saturating_sub(visible)))
        })
        .collect();
    padded.join(sep).trim_end().to_string()
}

fn visible_width(s: &str) -> usize {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for e in chars.by_ref() {
                if e == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out.width()
}

fn matches_group(item: &Item, ctx: &Ctx, key: &str, value: &str) -> bool {
    let f = resolve(item, ctx, key);
    if value.is_empty() {
        return f.is_missing();
    }
    f.values().iter().any(|v| v.eq_ignore_ascii_case(value))
}

/// Column headings, in a meaningful order: config order for statuses, due-date
/// order for milestones, first-seen otherwise.
fn column_values(ctx: &Ctx, items: &[Item], key: &str, all: bool) -> Vec<String> {
    let cfg = ctx.cfg;
    if key == "status" {
        return cfg
            .statuses
            .iter()
            .filter(|s| s.board && (all || !s.category.is_closed()))
            .map(|s| s.name.clone())
            .collect();
    }
    if key == "milestone" {
        let mut names: Vec<String> = cfg
            .milestones_ordered()
            .iter()
            .map(|m| m.name.clone())
            .collect();
        if items.iter().any(|i| i.milestone().is_none()) {
            names.push(String::new());
        }
        return names;
    }
    if let Some(def) = cfg.field(key)
        && !def.values.is_empty()
    {
        return def.values.clone();
    }
    let mut seen: Vec<String> = Vec::new();
    for i in items {
        for v in resolve(i, ctx, key).values() {
            if !seen.iter().any(|s| s == v) {
                seen.push(v.to_string());
            }
        }
    }
    seen
}
