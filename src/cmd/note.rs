// cairn — appending to an item's body.
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
use crate::item::parse_id;
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
    #[arg(long, action = ArgAction::SetTrue)]
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
    let mut item = store.find(parse_id(&args.id)?)?;

    let addition = if args.bare {
        text.to_string()
    } else {
        let heading = args.heading.clone().unwrap_or_else(today);
        format!("## {heading}\n\n{text}")
    };
    // One blank line between what was there and what is being added, whatever
    // the body ended with.
    let body = item.body.trim_end();
    item.body = if body.is_empty() {
        addition
    } else {
        format!("{body}\n\n{addition}")
    };
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
