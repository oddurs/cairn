// cairn — src/cmd/new.rs
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
// cairn new — create an item under the configured schema.
use crate::cmd::set::apply;
use crate::config::Config;
use crate::item::Item;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::{Assign, hooks, parse_assignment, style};
use anyhow::{Context, Result, bail};
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Item title
    #[arg(value_name = "TITLE")]
    pub title: String,

    /// Item type
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    pub kind: Option<String>,

    /// Initial status
    #[arg(short, long, value_name = "STATUS")]
    pub status: Option<String>,

    /// Milestone to file it under
    #[arg(short, long, value_name = "MILESTONE")]
    pub milestone: Option<String>,

    /// Labels (repeatable, or comma-separated)
    #[arg(short, long = "label", value_name = "LABEL", value_delimiter = ',')]
    pub labels: Vec<String>,

    /// Assignee
    #[arg(short, long, value_name = "WHO")]
    pub assignee: Option<String>,

    /// Items this one depends on
    #[arg(
        short = 'd',
        long = "depends-on",
        value_name = "ID",
        value_delimiter = ','
    )]
    pub depends_on: Vec<String>,

    /// Set any other field: --set priority=p0
    #[arg(long = "set", value_name = "FIELD=VALUE")]
    pub set: Vec<String>,

    /// Body text (defaults to the type's template)
    #[arg(short, long, value_name = "TEXT")]
    pub body: Option<String>,

    /// Read the body from stdin
    #[arg(long, action = ArgAction::SetTrue)]
    pub stdin: bool,

    /// Open the new item in $EDITOR
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub edit: bool,

    /// Print only the new id
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    // Held across allocation and write: reading the highest id and adding one
    // is only correct while nothing else is doing the same.
    let lock = Lock::acquire(&cfg)?;
    let existing = store.load_all()?;
    let id = store.next_id(&existing);
    let now = today();

    let mut item = Item {
        id,
        meta: Default::default(),
        body: String::new(),
        path: store.path_for(id, &args.title),
        front: String::new(),
        eol: Default::default(),
    };
    item.meta.title = Some(args.title.clone());
    item.meta.created = Some(now.clone());
    item.meta.updated = Some(now);

    let kind = args.kind.or_else(|| cfg.project.default_type.clone());
    if let Some(k) = kind {
        apply(&mut item, &cfg, "type", Assign::Set(k))?;
    }
    let status = args
        .status
        .unwrap_or_else(|| cfg.initial_status().to_string());
    apply(&mut item, &cfg, "status", Assign::Set(status))?;

    if let Some(m) = args.milestone {
        apply(&mut item, &cfg, "milestone", Assign::Set(m))?;
    }
    if let Some(a) = args.assignee {
        apply(&mut item, &cfg, "assignee", Assign::Set(a))?;
    }
    if !args.labels.is_empty() {
        apply(
            &mut item,
            &cfg,
            "labels",
            Assign::Set(args.labels.join(",")),
        )?;
    }
    if !args.depends_on.is_empty() {
        apply(
            &mut item,
            &cfg,
            "depends_on",
            Assign::Set(args.depends_on.join(",")),
        )?;
    }

    // Schema defaults come before explicit --set, so --set always wins.
    for f in &cfg.fields {
        if let Some(default) = &f.default {
            apply(&mut item, &cfg, &f.name, Assign::Set(default.clone()))?;
        }
    }
    for raw in &args.set {
        let (key, assign) = parse_assignment(raw)?;
        apply(&mut item, &cfg, &key, assign)?;
    }

    for f in &cfg.fields {
        if f.required && item.get(&f.name).is_missing() {
            bail!("field `{}` is required — pass --set {}=...", f.name, f.name);
        }
    }

    item.body = if args.stdin {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .context("reading body from stdin")?;
        buf
    } else if let Some(b) = args.body {
        b
    } else {
        item.kind()
            .and_then(|k| cfg.item_type(k))
            .and_then(|t| t.template.clone())
            .unwrap_or_default()
    };

    if item.path.exists() {
        bail!("{} already exists", item.path.display());
    }
    if !item.meta.depends_on.is_empty() {
        crate::cmd::set::check_no_cycle(&store, &item)?;
    }
    item.save()?;

    if args.quiet {
        println!("{}", cfg.format_id(item.id));
    } else {
        println!(
            "{} {}  {}",
            style::green("created"),
            style::bold(&cfg.format_id(item.id)),
            item.title()
        );
        println!("{}", style::dim(&store.rel(&item.path)));
    }

    // Released before hooks run. A hook may itself call cairn, and the write is
    // already durable by now, so there is nothing left to protect.
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterCreate, &item);

    if args.edit {
        crate::cmd::show::launch_editor(&item.path)?;
    }
    Ok(0)
}
