// cairn — appending to an item's body.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// A status records what was decided; a note records why. Without somewhere to
// put the reason, a backlog becomes a list of things nobody remembers
// rejecting — and an agent, which has no editor to fall back on, cannot record
// one at all.
//
// Deliberately append-only. Replacing a body is what `cairn edit` and the MCP
// update tool are for; a command whose job is to add to the record should not
// be able to erase it.
use crate::config::Config;
use crate::hooks;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::style;
use anyhow::{Context, Result, bail};
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Item id
    #[arg(value_name = "ID")]
    pub id: String,

    /// The note. Omit with --stdin to read it from standard input.
    #[arg(value_name = "TEXT")]
    pub text: Option<String>,

    /// Read the note from standard input
    #[arg(long, action = ArgAction::SetTrue)]
    pub stdin: bool,

    /// Heading to file it under (default: today's date)
    #[arg(long, value_name = "TEXT")]
    pub heading: Option<String>,

    /// Append without a heading
    #[arg(long, action = ArgAction::SetTrue, conflicts_with = "heading")]
    pub bare: bool,

    /// Print nothing on success
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);

    let text = match (&args.text, args.stdin) {
        (Some(t), false) => t.clone(),
        (None, true) => {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                .context("reading the note from standard input")?;
            buf
        }
        (Some(_), true) => bail!("give the note as an argument or on stdin, not both"),
        (None, false) => bail!("give the note as an argument, or --stdin to read it"),
    };
    let text = text.trim();
    if text.is_empty() {
        bail!("the note is empty");
    }

    let lock = Lock::acquire(&cfg)?;
    let mut item = store.find_ref(&args.id)?;

    match args.bare {
        true => {
            let body = item.body.trim_end();
            let combined = if body.is_empty() {
                text.to_string()
            } else {
                format!("{body}\n\n{text}")
            };
            item.set_body(&combined);
        }
        false => item.append_note(&args.heading.clone().unwrap_or_else(today), text),
    }
    item.touch(&today());
    item.save()?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &item);

    if !args.quiet {
        println!(
            "{} {}  {}",
            style::green("noted"),
            style::bold(&cfg.format_id(item.id)),
            item.title()
        );
    }
    Ok(0)
}
