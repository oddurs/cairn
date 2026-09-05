// cairn — claiming and releasing work.
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
// When more than one worker shares a backlog — two people, or two agents, or
// one of each — the scarce resource is agreement about who is doing what.
// Claiming writes that agreement into the item, where everyone can see it and
// git can merge it.
use crate::cmd::set::apply;
use crate::config::{Category, Config};
use crate::filter::Ctx;
use crate::item::parse_id;
use crate::lock::Lock;
use crate::store::{Store, today, whoami};
use crate::{Assign, hooks, style};
use anyhow::{Result, bail};
use clap::ArgAction;

#[derive(clap::Args)]
pub struct ClaimArgs {
    /// Item id; omit with --next to take the top-ranked ready item
    #[arg(value_name = "ID")]
    pub id: Option<String>,

    /// Claim whatever `cairn next` would suggest
    #[arg(long, action = ArgAction::SetTrue)]
    pub next: bool,

    /// Claim on behalf of someone else (default: you)
    #[arg(long = "as", value_name = "WHO")]
    pub who: Option<String>,

    /// Status to move to (default: the first `active` status)
    #[arg(short, long, value_name = "STATUS")]
    pub status: Option<String>,

    /// Take it even if someone else holds it
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub force: bool,

    /// Print only the claimed id
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

#[derive(clap::Args)]
pub struct ReleaseArgs {
    /// Item ids
    #[arg(value_name = "ID", required = true)]
    pub ids: Vec<String>,

    /// Status to move back to (default: the project's default status)
    #[arg(short, long, value_name = "STATUS")]
    pub status: Option<String>,

    /// Keep the current status, only drop the assignee
    #[arg(long, action = ArgAction::SetTrue)]
    pub keep_status: bool,

    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn claim(args: ClaimArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    // Taken before anything is read, so choosing an item, checking who holds it
    // and writing the claim are one indivisible step. Without it two claimers
    // can both pass the check before either writes.
    let lock = Lock::acquire(&cfg)?;
    let items = store.load_all()?;
    let ctx = Ctx::new(&cfg, &items);

    let id = match (&args.id, args.next) {
        (Some(raw), _) => parse_id(raw)?,
        (None, true) => {
            let picked = crate::cmd::next::select(
                &cfg,
                &ctx,
                &items,
                &crate::cmd::next::Args {
                    limit: 1,
                    assignee: None,
                    mine: false,
                    unassigned: true,
                    milestone: None,
                    kind: None,
                    filter: None,
                    blocked: false,
                    json: false,
                    ids: false,
                },
            )?;
            match picked.first() {
                Some(i) => i.id,
                None => {
                    eprintln!("{}", style::dim("nothing unclaimed is ready to start"));
                    return Ok(1);
                }
            }
        }
        (None, false) => bail!("give an item id, or --next to take the next ready one"),
    };

    let mut item = store.find(id)?;
    let who = args.who.clone().unwrap_or_else(whoami);

    if let Some(holder) = item.meta.assignee.as_deref()
        && !holder.is_empty()
        && !holder.eq_ignore_ascii_case(&who)
        && !args.force
    {
        bail!(
            "{} is already claimed by {holder}\nuse --force to take it anyway",
            cfg.format_id(id)
        );
    }
    if ctx.is_closed(&item) && !args.force {
        bail!(
            "{} is already {} — reopen it first, or --force",
            cfg.format_id(id),
            item.status()
        );
    }
    let blockers = ctx.blockers(&item);
    if !blockers.is_empty() && !args.force {
        let list: Vec<String> = blockers.iter().map(|b| cfg.format_id(*b)).collect();
        bail!(
            "{} is blocked by {}\nfinish those first, or --force",
            cfg.format_id(id),
            list.join(", ")
        );
    }

    let status = match &args.status {
        Some(s) => s.clone(),
        None => match cfg.statuses.iter().find(|s| s.category == Category::Active) {
            Some(s) => s.name.clone(),
            None => bail!(
                "no status with category = \"active\" is defined in cairn.toml; \
                 pass --status explicitly"
            ),
        },
    };

    apply(&mut item, &cfg, "assignee", Assign::Set(who.clone()))?;
    apply(&mut item, &cfg, "status", Assign::Set(status))?;
    item.touch(&today());
    item.save()?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &item);

    if args.quiet {
        println!("{}", cfg.format_id(item.id));
    } else {
        println!(
            "{} {}  {}",
            style::green("claimed"),
            style::bold(&cfg.format_id(item.id)),
            item.title()
        );
        println!(
            "{}",
            style::dim(&format!(
                "  {} · {} · {}",
                who,
                item.status(),
                store.rel(&item.path)
            ))
        );
    }
    Ok(0)
}

pub fn release(args: ReleaseArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let lock = Lock::acquire(&cfg)?;
    let mut released = Vec::new();
    for raw in &args.ids {
        let mut item = store.find(parse_id(raw)?)?;
        apply(&mut item, &cfg, "assignee", Assign::Set(String::new()))?;
        if !args.keep_status {
            let status = args
                .status
                .clone()
                .unwrap_or_else(|| cfg.initial_status().to_string());
            apply(&mut item, &cfg, "status", Assign::Set(status))?;
        }
        item.touch(&today());
        item.save()?;
        if !args.quiet {
            println!(
                "{} {}  {}",
                style::green("released"),
                style::bold(&cfg.format_id(item.id)),
                item.title()
            );
        }
        released.push(item);
    }
    drop(lock);

    for item in &released {
        hooks::item(&cfg, &store, hooks::Event::AfterChange, item);
    }
    Ok(0)
}
