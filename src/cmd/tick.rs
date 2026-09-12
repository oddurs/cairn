// cairn — ticking and unticking acceptance criteria.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// The criteria are the most-touched part of an item in a working loop: you tick
// them as you satisfy them, and they are what says whether the item is actually
// done. Everything needed to *read* them was already here — `Item::criteria`
// parses the boxes, `cairn show` counts them, the `criteria` filter keys query
// them, `require_criteria` makes `check` enforce them — and there was no way to
// write one. `set` takes fields, `note` appends, `edit` wants a terminal.
//
// So the one thing done most often was the one thing with no command, and the
// workaround was rewriting the Markdown from outside cairn, which is how
// somebody truncated an item to zero bytes.
use crate::config::Config;
use crate::hooks;
use crate::item::Criterion;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::style;
use anyhow::{Result, bail};
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Item id
    #[arg(value_name = "ID")]
    pub id: String,

    /// Which criteria, counting from 1. Omit with --all.
    #[arg(value_name = "N")]
    pub which: Vec<usize>,

    /// Every criterion the item states
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub all: bool,

    /// Print nothing on success
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn tick(args: Args) -> Result<i32> {
    run(args, true)
}

pub fn untick(args: Args) -> Result<i32> {
    run(args, false)
}

fn run(args: Args, ticked: bool) -> Result<i32> {
    let verb = if ticked { "tick" } else { "untick" };
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);

    // One of the two, never both and never neither.
    let numbered = !args.which.is_empty();
    if args.all == numbered {
        bail!("say which criteria to {verb} — their numbers, or --all");
    }

    let lock = Lock::acquire(&cfg)?;
    let mut item = store.find_ref(&args.id)?;
    let section = cfg.project.criteria_section.as_deref();
    let list = item.criteria_list(section);

    if list.is_empty() {
        bail!(
            "{} states no acceptance criteria{}",
            cfg.format_id(item.id),
            match section {
                Some(s) => format!(" under `## {s}`"),
                None => String::new(),
            }
        );
    }

    // Resolved before anything is written, so a command naming one criterion
    // that is not there changes none of the ones that are.
    let chosen: Vec<&Criterion> = if args.all {
        list.iter().collect()
    } else {
        let mut out = Vec::new();
        for n in &args.which {
            match n.checked_sub(1).and_then(|i| list.get(i)) {
                Some(c) => out.push(c),
                None => bail!(
                    "{} states {} acceptance criteria, so there is no {n}{} — \
                     `cairn show {} --criteria` lists them",
                    cfg.format_id(item.id),
                    list.len(),
                    if *n == 0 { " (they count from 1)" } else { "" },
                    cfg.format_id(item.id)
                ),
            }
        }
        out
    };

    let lines: Vec<usize> = chosen.iter().map(|c| c.line).collect();
    let already = chosen.iter().filter(|c| c.ticked == ticked).count();
    // Ticking what is already ticked is what a retried script does, and a tool
    // that failed there would make the retry the dangerous path. Nothing is
    // written, `updated` does not move, and no hook fires.
    if already == chosen.len() {
        if !args.quiet {
            println!(
                "{}  {} already {}ed",
                style::dim(&cfg.format_id(item.id)),
                chosen.len(),
                verb
            );
        }
        return Ok(0);
    }

    item.set_criteria(&lines, ticked);
    item.touch(&today());
    item.save()?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &item);

    if !args.quiet {
        let now = item.criteria(section);
        println!(
            "{} {}  {}",
            style::green(&format!("{verb}ed")),
            style::bold(&cfg.format_id(item.id)),
            item.title()
        );
        let count = format!("{} of {} criteria ticked", now.done, now.total);
        println!(
            "  {}",
            if now.complete() {
                style::green(&count)
            } else {
                style::dim(&count)
            }
        );
    }
    Ok(0)
}

/// The criteria, numbered the way `tick` numbers them.
///
/// Printed by `cairn show --criteria`: the numbers are only useful if they are
/// the same numbers, which is why both sides read `Item::criteria_list`.
pub fn print_criteria(list: &[Criterion]) {
    let width = list.len().to_string().len();
    for (n, c) in list.iter().enumerate() {
        // Padded before it is coloured: the escape sequences are bytes the
        // terminal does not draw, and counting them in the width right-aligns
        // the column to the wrong place the moment colour is on.
        let n = format!("{:>width$}", n + 1, width = width);
        println!(
            "  {}  {} {}",
            style::dim(&n),
            if c.ticked {
                style::green("[x]")
            } else {
                style::dim("[ ]")
            },
            c.text,
        );
    }
}
