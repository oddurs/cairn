// cairn — exporting the backlog.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
use crate::config::Config;
use crate::filter::{Ctx, Filter, sort_items};
use crate::item::Item;
use crate::store::{Store, today};
use crate::style;
use anyhow::{Result, bail};
use clap::{ArgAction, ValueEnum};

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// The documented cairn interchange document
    Json,
}

#[derive(clap::Args)]
pub struct Args {
    /// Output format
    #[arg(long = "to", value_enum, default_value = "json")]
    pub format: Format,

    /// Write to FILE instead of standard output
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<String>,

    /// Restrict to matching items
    #[arg(short, long, value_name = "EXPR")]
    pub filter: Option<String>,

    /// Exclude done and dropped items
    #[arg(long, action = ArgAction::SetTrue)]
    pub open_only: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let all = store.load_all()?;
    let ctx = Ctx::new(&cfg, &all);

    let filter = match &args.filter {
        Some(expr) => Filter::parse(expr)?,
        None => Filter::default(),
    };
    let mut items: Vec<Item> = all
        .iter()
        .filter(|i| filter.matches(i, &ctx))
        .filter(|i| !args.open_only || !ctx.is_closed(i))
        .cloned()
        .collect();
    sort_items(&mut items, "id", &ctx);

    let doc = crate::interchange::document(&cfg, &store, &items, &today());
    let text = serde_json::to_string_pretty(&doc)?;

    match &args.output {
        None => println!("{text}"),
        Some(path) if path == "-" => println!("{text}"),
        Some(path) => {
            let target = cfg.root.join(path);
            crate::store::write_atomic(&target, format!("{text}\n").as_bytes())?;
            eprintln!(
                "{} {}  {}",
                style::green("wrote"),
                store.rel(&target),
                style::dim(&format!("{} items", items.len()))
            );
        }
    }
    Ok(0)
}

/// Shared by import: `--map kind:from=to`.
pub fn parse_map(specs: &[String]) -> Result<std::collections::HashMap<(String, String), String>> {
    let mut out = std::collections::HashMap::new();
    for spec in specs {
        let Some((lhs, to)) = spec.split_once('=') else {
            bail!("`{spec}`: expected kind:from=to, e.g. status:open=backlog");
        };
        let Some((kind, from)) = lhs.split_once(':') else {
            bail!("`{spec}`: expected kind:from=to, e.g. status:open=backlog");
        };
        let kind = kind.trim().to_lowercase();
        if !matches!(
            kind.as_str(),
            "status" | "type" | "milestone" | "label" | "field"
        ) {
            bail!("`{spec}`: kind must be status, type, milestone, label or field");
        }
        out.insert((kind, from.trim().to_lowercase()), to.trim().to_string());
    }
    Ok(out)
}
