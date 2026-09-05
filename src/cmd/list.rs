// cairn — src/cmd/list.rs
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
// cairn list — the workhorse query command.
use crate::cmd::{item_json, paint_status, paint_type, status_text, type_text};
use crate::config::Config;
use crate::filter::{Ctx, Filter, Op, resolve, sort_items};
use crate::item::Item;
use crate::store::Store;
use crate::style;
use crate::table::{Cell, Table};
use anyhow::{Result, bail};
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Filter by status (repeatable)
    #[arg(short, long, value_name = "STATUS", value_delimiter = ',')]
    pub status: Vec<String>,

    /// Filter by type (repeatable)
    #[arg(short = 't', long = "type", value_name = "TYPE", value_delimiter = ',')]
    pub kind: Vec<String>,

    /// Filter by milestone (repeatable)
    #[arg(short, long, value_name = "MILESTONE", value_delimiter = ',')]
    pub milestone: Vec<String>,

    /// Filter by label (repeatable)
    #[arg(short, long = "label", value_name = "LABEL", value_delimiter = ',')]
    pub labels: Vec<String>,

    /// Filter by assignee
    #[arg(short, long, value_name = "WHO")]
    pub assignee: Option<String>,

    /// Raw filter expression, e.g. 'priority=p0,category!=done'
    #[arg(short, long, value_name = "EXPR")]
    pub filter: Option<String>,

    /// Use a saved view from cairn.toml
    #[arg(long, value_name = "NAME")]
    pub view: Option<String>,

    /// Sort keys, `-` for descending: 'priority,-updated'
    #[arg(long, value_name = "KEYS")]
    pub sort: Option<String>,

    /// Columns to show
    #[arg(long, value_name = "FIELDS", value_delimiter = ',')]
    pub columns: Vec<String>,

    /// Include done and dropped items
    #[arg(short = 'A', long, action = ArgAction::SetTrue)]
    pub all: bool,

    /// Show at most N items
    #[arg(short = 'n', long, value_name = "N")]
    pub limit: Option<usize>,

    /// Output JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,

    /// Output ids only, one per line
    #[arg(long, action = ArgAction::SetTrue)]
    pub ids: bool,

    /// Tab-separated, no header or colour
    #[arg(long, action = ArgAction::SetTrue)]
    pub plain: bool,

    /// Print the number of matches only
    #[arg(long, action = ArgAction::SetTrue)]
    pub count: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let mut items = store.load_for_reading()?;

    let view = match &args.view {
        Some(name) => match cfg.view(name) {
            Some(v) => Some(v),
            None => bail!(
                "unknown view `{name}`\nknown views: {}",
                if cfg.views.is_empty() {
                    "(none defined)".to_string()
                } else {
                    cfg.views
                        .iter()
                        .map(|v| v.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
        },
        None => None,
    };

    let filter = build_filter(&args, view)?;
    let mentions_status = filter
        .clauses
        .iter()
        .any(|c| matches!(c.key.as_str(), "status" | "category"));

    // Built from the full set before filtering, so `blocked` still reflects
    // dependencies the filter itself excluded.
    let ctx = Ctx::new(&cfg, &items);
    items.retain(|i| filter.matches(i, &ctx));
    // Closed items are hidden by default, but never when the caller has said
    // something about status themselves.
    if !args.all && !mentions_status {
        items.retain(|i| !cfg.category(i.status()).is_closed());
    }

    let sort = args
        .sort
        .clone()
        .or_else(|| view.and_then(|v| v.sort.clone()))
        .unwrap_or_else(|| "milestone,status,id".to_string());
    sort_items(&mut items, &sort, &ctx);

    if let Some(n) = args.limit {
        items.truncate(n);
    }

    if args.count {
        println!("{}", items.len());
        return Ok(0);
    }
    if args.ids {
        for i in &items {
            println!("{}", cfg.format_id(i.id));
        }
        return Ok(0);
    }
    if args.json {
        let arr: Vec<_> = items
            .iter()
            .map(|i| item_json(&cfg, i, &store, false))
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(0);
    }

    let columns = resolve_columns(&args, view, &cfg);
    if items.is_empty() {
        if !args.plain {
            eprintln!("{}", style::dim("no items match"));
        }
        return Ok(0);
    }
    if args.plain {
        for i in &items {
            let cells: Vec<String> = columns.iter().map(|c| machine(&ctx, i, c)).collect();
            println!("{}", cells.join("\t"));
        }
        return Ok(0);
    }

    let headers: Vec<&str> = columns.iter().map(String::as_str).collect();
    let mut t = Table::new(&headers);
    for i in &items {
        t.row(columns.iter().map(|c| cell(&ctx, i, c)).collect());
    }
    print!("{}", t.render());
    Ok(0)
}

pub fn build_filter(args: &Args, view: Option<&crate::config::View>) -> Result<Filter> {
    let mut f = Filter::default();
    if let Some(v) = view
        && let Some(expr) = &v.filter
    {
        f = f.and(Filter::parse(expr)?);
    }
    if !args.status.is_empty() {
        f.push("status", Op::Eq, args.status.clone());
    }
    if !args.kind.is_empty() {
        f.push("type", Op::Eq, args.kind.clone());
    }
    if !args.milestone.is_empty() {
        f.push("milestone", Op::Eq, args.milestone.clone());
    }
    for l in &args.labels {
        f.push("labels", Op::Eq, vec![l.clone()]);
    }
    if let Some(a) = &args.assignee {
        f.push("assignee", Op::Eq, vec![a.clone()]);
    }
    if let Some(expr) = &args.filter {
        f = f.and(Filter::parse(expr)?);
    }
    Ok(f)
}

fn resolve_columns(args: &Args, view: Option<&crate::config::View>, cfg: &Config) -> Vec<String> {
    if !args.columns.is_empty() {
        return args.columns.clone();
    }
    if let Some(v) = view
        && !v.columns.is_empty()
    {
        return v.columns.clone();
    }
    let mut cols = vec!["id".to_string(), "status".to_string()];
    if !cfg.types.is_empty() {
        cols.push("type".into());
    }
    if !cfg.milestones.is_empty() {
        cols.push("milestone".into());
    }
    for f in cfg.fields.iter().filter(|f| f.column) {
        cols.push(f.name.clone());
    }
    // Title last: the table gives the final column whatever width is left.
    cols.push("title".into());
    cols
}

/// Column text for a person reading a terminal: statuses and types appear as
/// their labels and icons, which is what the schema declared them to look like.
fn display(ctx: &Ctx, item: &Item, column: &str) -> String {
    let cfg = ctx.cfg;
    match column {
        "id" => cfg.format_id(item.id),
        "title" => item.title().to_string(),
        "status" => status_text(cfg, item.status()),
        "type" => type_text(cfg, item.kind()),
        "summary" => item.summary(),
        _ => resolve(item, ctx, column).display(),
    }
}

/// Column text for a program. `--plain` exists to be piped into `cut`, `awk` or
/// `grep`, so it emits the names a filter would accept — not the labels a
/// person is shown. `cairn list --plain --columns status | grep doing` has to
/// find the items whose status is `doing`, whatever the schema calls it.
fn machine(ctx: &Ctx, item: &Item, column: &str) -> String {
    match column {
        "id" => ctx.cfg.format_id(item.id),
        "title" => item.title().to_string(),
        "status" => item.status().to_string(),
        "type" => item.kind().unwrap_or("").to_string(),
        "summary" => item.summary(),
        _ => resolve(item, ctx, column).display(),
    }
}

fn cell(ctx: &Ctx, item: &Item, column: &str) -> Cell {
    let text = display(ctx, item, column);
    match column {
        "id" => Cell::styled(&text, style::dim(&text)),
        "status" => Cell::styled(&text, paint_status(ctx.cfg, item.status())),
        "type" => Cell::styled(&text, paint_type(ctx.cfg, item.kind())),
        _ => Cell::plain(text),
    }
}
