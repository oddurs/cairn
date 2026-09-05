// cairn — finding the next thing to work on.
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
// The question an agent (or a person) opens the backlog with is "what should I
// do now?", and answering it from `list` requires knowing the schema, composing
// a filter, and resolving dependencies by hand. This command is that question.
use crate::cmd::{item_json, paint_status};
use crate::config::{Category, Config};
use crate::filter::{Ctx, Filter, Op, sort_items};
use crate::item::Item;
use crate::store::Store;
use crate::style;
use crate::table::{Cell, Table};
use anyhow::Result;
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Show at most N items
    #[arg(short = 'n', long, value_name = "N", default_value = "5")]
    pub limit: usize,

    /// Only work assigned to WHO
    #[arg(short, long, value_name = "WHO")]
    pub assignee: Option<String>,

    /// Only work assigned to you, or to nobody
    #[arg(long, action = ArgAction::SetTrue)]
    pub mine: bool,

    /// Only work with no assignee
    #[arg(long, action = ArgAction::SetTrue)]
    pub unassigned: bool,

    /// Restrict to a milestone
    #[arg(short, long, value_name = "MILESTONE")]
    pub milestone: Option<String>,

    /// Restrict to a type
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    pub kind: Option<String>,

    /// Additional filter expression
    #[arg(short, long, value_name = "EXPR")]
    pub filter: Option<String>,

    /// Include blocked work, annotated with what blocks it
    #[arg(short = 'b', long, action = ArgAction::SetTrue)]
    pub blocked: bool,

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
    let picked = select(&cfg, &ctx, &items, &args)?;

    if args.ids {
        for i in &picked {
            println!("{}", cfg.format_id(i.id));
        }
        return Ok(0);
    }
    if args.json {
        let arr: Vec<_> = picked
            .iter()
            .map(|i| {
                let mut v = item_json(&cfg, i, &store, false);
                if let Some(o) = v.as_object_mut() {
                    o.insert("blockers".into(), serde_json::json!(ctx.blockers(i)));
                    o.insert("ready".into(), serde_json::json!(ctx.is_ready(i)));
                }
                v
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(0);
    }

    if picked.is_empty() {
        let blocked = items.iter().filter(|i| ctx.is_blocked(i)).count();
        eprintln!("{}", style::dim("nothing is ready to start"));
        if blocked > 0 && !args.blocked {
            eprintln!(
                "{}",
                style::dim(&format!(
                    "{blocked} item(s) are blocked — `cairn next --blocked` to see what by"
                ))
            );
        }
        return Ok(0);
    }

    let mut t = Table::new(&["id", "status", "milestone", "blocked by", "title"]);
    for i in &picked {
        let blockers = ctx.blockers(i);
        let blocked_by = blockers
            .iter()
            .map(|b| cfg.format_id(*b))
            .collect::<Vec<_>>()
            .join(",");
        let status = cfg
            .status(i.status())
            .map(|s| s.display().to_string())
            .unwrap_or_else(|| i.status().to_string());
        let id = cfg.format_id(i.id);
        t.row(vec![
            Cell::styled(id.clone(), style::dim(&id)),
            Cell::styled(&status, paint_status(&cfg, i.status())),
            Cell::plain(i.milestone().unwrap_or("")),
            Cell::styled(&blocked_by, style::yellow(&blocked_by)),
            Cell::plain(i.title()),
        ]);
    }
    print!("{}", t.render());
    Ok(0)
}

/// Ranked, filtered candidates. Shared with the MCP server so both surfaces
/// answer "what next?" identically.
pub fn select<'a>(
    cfg: &Config,
    ctx: &Ctx,
    items: &'a [Item],
    args: &Args,
) -> Result<Vec<&'a Item>> {
    let mut filter = Filter::default();
    if let Some(m) = &args.milestone {
        filter.push("milestone", Op::Eq, vec![m.clone()]);
    }
    if let Some(k) = &args.kind {
        filter.push("type", Op::Eq, vec![k.clone()]);
    }
    if let Some(a) = &args.assignee {
        filter.push("assignee", Op::Eq, vec![a.clone()]);
    }
    if args.unassigned {
        filter.push("assignee", Op::Eq, vec![String::new()]);
    }
    if let Some(expr) = &args.filter {
        filter = filter.and(Filter::parse(expr)?);
    }

    let me = crate::store::whoami();
    let mut chosen: Vec<Item> = items
        .iter()
        .filter(|i| !ctx.is_closed(i))
        .filter(|i| args.blocked || !ctx.is_blocked(i))
        .filter(|i| filter.matches(i, ctx))
        // `--mine` means work nobody else has taken: mine, or unclaimed.
        .filter(|i| {
            !args.mine
                || match i.meta.assignee.as_deref() {
                    None | Some("") => true,
                    Some(a) => a.eq_ignore_ascii_case(&me),
                }
        })
        .cloned()
        .collect();

    // Priority first if the schema has one, then milestone order, then id.
    let spec = if cfg.field("priority").is_some() {
        "priority,milestone,id"
    } else {
        "milestone,id"
    };
    sort_items(&mut chosen, spec, ctx);
    // Work already under way outranks work not yet started: the most useful
    // next action is usually finishing something. Sort is stable, so this
    // reorders the two groups without disturbing the ranking within them.
    chosen.sort_by_key(|i| cfg.category(i.status()) != Category::Active);

    let ids: Vec<u32> = chosen.iter().take(args.limit).map(|i| i.id).collect();
    Ok(ids
        .iter()
        .filter_map(|id| items.iter().find(|i| i.id == *id))
        .collect())
}
