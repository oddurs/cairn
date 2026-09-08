// cairn — proposing a change somebody else decides.
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
// `agent = "propose"` already refused the write and told the caller to explain
// itself in a note. Then the proposal was prose in a body, and a person wanting
// to review proposals had to read every note in the backlog to find them. The
// hard half was built; the place for the answer to go was missing.
//
// A proposal is a note with structure. It lives in the body like every other
// reason in this project, because a proposal that vanished when somebody edited
// frontmatter would be worse than prose — and because three proposals on one
// field is a conversation, not a value.
use crate::config::Config;
use crate::item::Item;
use crate::lock::Lock;
use crate::store::{Store, today, whoami};
use crate::style;
use crate::{Assign, hooks};
use anyhow::{Result, bail};
use clap::ArgAction;

/// The heading a proposal is filed under. Parsed back out, so it is a shape
/// rather than a convention somebody has to honour by hand.
const HEADING: &str = "Proposed";

#[derive(clap::Args)]
pub struct Args {
    /// Item id
    #[arg(value_name = "ID")]
    pub id: String,

    /// The change being proposed, as `field=value`
    #[arg(value_name = "FIELD=VALUE")]
    pub assignment: String,

    /// Why. A proposal without one is a preference, not an argument.
    #[arg(long, value_name = "TEXT")]
    pub why: Option<String>,

    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

#[derive(clap::Args)]
pub struct ListArgs {
    /// Apply this item's most recent proposal, as you
    #[arg(long, value_name = "ID")]
    pub accept: Option<String>,

    /// Machine-readable output
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

/// One proposal, as it was written down.
#[derive(Debug, Clone)]
pub struct Proposal {
    pub id: u32,
    pub field: String,
    pub from: String,
    pub to: String,
    pub who: String,
    pub when: String,
    pub why: String,
}

impl Proposal {
    /// `priority: p2 -> p0`, which is the line a person reads first.
    pub fn change(&self) -> String {
        let from = if self.from.is_empty() {
            "unset".to_string()
        } else {
            self.from.clone()
        };
        format!("{}: {from} -> {}", self.field, self.to)
    }
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let lock = Lock::acquire(&cfg)?;
    let mut item = store.find_ref(&args.id)?;

    let (field, value) = args
        .assignment
        .split_once('=')
        .map(|(f, v)| (f.trim().to_string(), v.trim().to_string()))
        .filter(|(f, _)| !f.is_empty())
        .ok_or_else(|| anyhow::anyhow!("a proposal is `field=value`, e.g. priority=p0"))?;

    // Proposing something that would be refused as a write is the whole point,
    // so the value is checked against the schema but the permission is not.
    let mut trial = item.clone();
    crate::cmd::set::apply(&mut trial, &cfg, &field, Assign::Set(value.clone()))?;

    let from = item.get(&field).display();
    if from == value {
        bail!("`{field}` is already `{value}`");
    }

    let why = args.why.clone().unwrap_or_default();
    let who = whoami();
    item.append_note(
        &format!("{HEADING} {field}: {from} -> {value} ({who}, {})", today()),
        if why.is_empty() {
            "No reason given."
        } else {
            &why
        },
    );
    item.touch(&today());
    item.save()?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &item);

    if !args.quiet {
        println!(
            "{} {}  {}",
            style::green("proposed"),
            style::bold(&cfg.format_id(item.id)),
            style::dim(&format!(
                "{field}: {} -> {value}",
                if from.is_empty() { "unset" } else { &from }
            ))
        );
        println!(
            "{}",
            style::dim("a person decides — `cairn proposals` lists what is waiting")
        );
    }
    Ok(0)
}

/// Every proposal on an item, oldest first.
pub fn of(item: &Item) -> Vec<Proposal> {
    let mut out = Vec::new();
    let mut current: Option<Proposal> = None;
    let mut why = String::new();

    for line in item.body.lines() {
        if let Some(heading) = line.trim().strip_prefix("## ") {
            if let Some(mut p) = current.take() {
                p.why = why.trim().to_string();
                out.push(p);
            }
            why.clear();
            current = parse_heading(item.id, heading);
            continue;
        }
        if current.is_some() {
            why.push_str(line);
            why.push('\n');
        }
    }
    if let Some(mut p) = current.take() {
        p.why = why.trim().to_string();
        out.push(p);
    }
    out
}

/// `Proposed priority: p2 -> p0 (claude, 2026-09-08)`
fn parse_heading(id: u32, heading: &str) -> Option<Proposal> {
    let rest = heading.trim().strip_prefix(HEADING)?.trim();
    let (change, who_when) = rest.rsplit_once('(')?;
    let who_when = who_when.trim_end_matches(')');
    let (who, when) = who_when.rsplit_once(", ")?;
    let (field, values) = change.split_once(':')?;
    let (from, to) = values.split_once("->")?;
    Some(Proposal {
        id,
        field: field.trim().to_string(),
        from: from.trim().trim_matches(|c| c == '`').to_string(),
        to: to.trim().to_string(),
        who: who.trim().to_string(),
        when: when.trim().to_string(),
        why: String::new(),
    })
}

pub fn list(args: ListArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);

    if let Some(raw) = &args.accept {
        return accept(&cfg, &store, raw);
    }

    let items = store.load_all()?;
    let mut all: Vec<Proposal> = items.iter().flat_map(of).collect();
    // Still open only: a proposal already applied is history, and the question
    // this command answers is what is waiting.
    all.retain(|p| {
        items
            .iter()
            .find(|i| i.id == p.id)
            .is_some_and(|i| i.get(&p.field).display() != p.to)
    });

    if args.json {
        let arr: Vec<_> = all
            .iter()
            .map(|p| {
                serde_json::json!({
                    "id": p.id, "ref": cfg.format_id(p.id), "field": p.field,
                    "from": p.from, "to": p.to, "by": p.who, "when": p.when, "why": p.why,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(0);
    }

    if all.is_empty() {
        eprintln!("{}", style::dim("nothing is proposed"));
        return Ok(0);
    }
    for p in &all {
        println!(
            "{}  {}   {}",
            style::dim(&cfg.format_id(p.id)),
            style::bold(&p.change()),
            style::dim(&format!("{}, {}", p.who, p.when))
        );
        if !p.why.is_empty() {
            for line in p.why.lines() {
                println!("      {line}");
            }
        }
    }
    println!();
    println!(
        "{}",
        style::dim("`cairn proposals --accept <ID>` applies the most recent one")
    );
    Ok(0)
}

/// Apply an item's most recent open proposal, as the person running this.
///
/// Recorded rather than silently made, so the change does not later look as
/// though it was always so.
fn accept(cfg: &Config, store: &Store, raw: &str) -> Result<i32> {
    let lock = Lock::acquire(cfg)?;
    let mut item = store.find_ref(raw)?;
    let open: Vec<Proposal> = of(&item)
        .into_iter()
        .filter(|p| item.get(&p.field).display() != p.to)
        .collect();
    let Some(p) = open.last() else {
        bail!("{} has nothing proposed", cfg.format_id(item.id));
    };

    crate::cmd::set::apply(&mut item, cfg, &p.field, Assign::Set(p.to.clone()))?;
    let who = whoami();
    item.append_note(
        &format!("Accepted {}: {} ({who}, {})", p.field, p.to, today()),
        &format!("Proposed by {} on {}.", p.who, p.when),
    );
    item.touch(&today());
    item.save()?;
    store.sync_path(&mut item)?;
    drop(lock);
    hooks::item(cfg, store, hooks::Event::AfterChange, &item);

    println!(
        "{} {}  {}",
        style::green("accepted"),
        style::bold(&cfg.format_id(item.id)),
        style::dim(&format!("{} = {}", p.field, p.to))
    );
    Ok(0)
}
