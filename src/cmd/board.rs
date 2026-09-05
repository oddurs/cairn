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

    // A card carries what the list carries: the id, whatever the schema thinks
    // is worth a column, and the title. Blocked work is marked, because "cannot
    // be started" is the most useful thing to know at a glance and was
    // previously invisible.
    let fields = crate::cmd::table_fields(&cfg);
    let any_blocked = items.iter().any(|i| ctx.is_blocked(i));
    let marker = |i: &Item| -> &str {
        if !any_blocked {
            ""
        } else if ctx.is_blocked(i) {
            "! "
        } else {
            "  "
        }
    };

    let mut plain: Vec<Vec<String>> = Vec::with_capacity(n);
    let mut painted: Vec<Vec<String>> = Vec::with_capacity(n);
    let mut headers: Vec<String> = Vec::with_capacity(n);
    let mut header_plain: Vec<String> = Vec::with_capacity(n);

    for value in &columns {
        let members: Vec<&Item> = items
            .iter()
            .filter(|i| matches_group(i, &ctx, &group_by, value))
            .collect();

        let label = if group_by == "status" {
            crate::cmd::status_text(&cfg, value)
        } else if value.is_empty() {
            "(none)".to_string()
        } else {
            value.clone()
        };
        let heading = format!("{label} {}", members.len());
        header_plain.push(heading.clone());
        headers.push(if group_by == "status" {
            format!(
                "{} {}",
                paint_status(&cfg, value),
                style::dim(&members.len().to_string())
            )
        } else {
            format!("{label} {}", style::dim(&members.len().to_string()))
        });

        let mut col_plain = Vec::with_capacity(members.len());
        let mut col_painted = Vec::with_capacity(members.len());
        for i in &members {
            let id = cfg.format_id(i.id);
            let extras: Vec<String> = fields
                .iter()
                .map(|f| resolve(i, &ctx, f).display())
                .filter(|v| !v.is_empty())
                .collect();
            let prefix = if extras.is_empty() {
                format!("{}{} ", marker(i), id)
            } else {
                format!("{}{} {} ", marker(i), id, extras.join(" "))
            };
            col_plain.push(format!("{prefix}{}", i.title()));
            col_painted.push(prefix);
            col_painted.pop();
            col_painted.push(format!(
                "{}{} {}",
                if any_blocked && ctx.is_blocked(i) {
                    style::yellow("!")
                } else if any_blocked {
                    " ".to_string()
                } else {
                    String::new()
                },
                style::dim(&id),
                if extras.is_empty() {
                    i.title().to_string()
                } else {
                    format!("{} {}", extras.join(" "), i.title())
                }
            ));
        }
        plain.push(col_plain);
        painted.push(col_painted);
    }

    // Width in proportion to content rather than in equal shares. An empty
    // column needs its heading and nothing more; a full one should not be
    // truncated to match it.
    let want: Vec<usize> = (0..n)
        .map(|i| {
            plain[i]
                .iter()
                .map(|c| c.width())
                .chain(std::iter::once(header_plain[i].width()))
                .max()
                .unwrap_or(8)
        })
        .collect();
    let available = term.saturating_sub(gap * n.saturating_sub(1));
    // A column narrower than an id plus an ellipsis shows nothing at all, so a
    // requested width is clamped rather than obeyed literally.
    let floor = cfg.project.id_width + 4;
    let widths: Vec<usize> = if let Some(w) = args.width {
        vec![w.max(floor); n]
    } else if want.iter().sum::<usize>() <= available {
        want
    } else {
        // Everything shrinks together, but nothing below what a heading needs.
        let total: usize = want.iter().sum();
        want.iter()
            .enumerate()
            .map(|(i, w)| {
                let share = w * available / total.max(1);
                share.max(header_plain[i].width().min(14)).max(floor)
            })
            .collect()
    };

    let sep = " ".repeat(gap);
    println!("{}", join_columns(&headers, &widths, &sep));
    let rule: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    println!("{}", style::dim(&join_columns(&rule, &widths, &sep)));

    let depth = painted.iter().map(Vec::len).max().unwrap_or(0);
    for row in 0..depth {
        let line: Vec<String> = (0..n)
            .map(|c| {
                painted[c]
                    .get(row)
                    .map(|text| clip_visible(text, widths[c]))
                    .unwrap_or_default()
            })
            .collect();
        println!("{}", join_columns(&line, &widths, &sep));
    }

    let all: Vec<&Item> = items.iter().collect();
    println!("\n{}", style::dim(&crate::cmd::summary(&ctx, &all)));
    Ok(0)
}

/// Pad each cell to its own column's width, measuring what the reader sees
/// rather than the escape codes.
fn join_columns(cells: &[String], widths: &[usize], sep: &str) -> String {
    let padded: Vec<String> = cells
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let w = widths.get(i).copied().unwrap_or(0);
            format!("{c}{}", " ".repeat(w.saturating_sub(visible_width(c))))
        })
        .collect();
    padded.join(sep).trim_end().to_string()
}

/// Truncate to a visible width, keeping escape sequences intact.
fn clip_visible(text: &str, max: usize) -> String {
    if visible_width(text) <= max {
        return text.to_string();
    }
    let mut out = String::new();
    let mut shown = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            out.push(c);
            for e in chars.by_ref() {
                out.push(e);
                if e == 'm' {
                    break;
                }
            }
            continue;
        }
        let w = c.to_string().width();
        if shown + w > max.saturating_sub(1) {
            break;
        }
        out.push(c);
        shown += w;
    }
    out.push('…');
    // Only close a sequence that was actually opened: appending a reset
    // unconditionally would put an escape into --color never output, which is
    // meant to be plain text a script can read.
    if out.contains('\u{1b}') {
        out.push_str("\u{1b}[0m");
    }
    out
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
