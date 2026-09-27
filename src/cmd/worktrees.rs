// cairn — what the other worktrees are doing with the backlog.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// `next` leaves out what another worktree holds, and says how much it left
// out. This is where that work is shown: every item another worktree of this
// repository has changed or filed since it diverged from this one, as that
// worktree has it. Items filed there are included — they are half of what is
// going on, and nothing in this checkout can see them otherwise.
use crate::cmd::paint_status;
use crate::config::Config;
use crate::store::Store;
use crate::style;
use crate::worktree::{Survey, held, taken};
use anyhow::Result;
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// Output JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_for_reading()?;
    let survey = Survey::take(&cfg, &items);

    if args.json {
        let others: Vec<_> = survey
            .others
            .iter()
            .map(|o| {
                serde_json::json!({
                    "branch": o.branch,
                    "path": o.path,
                    "items": o.copies.iter().map(|c| serde_json::json!({
                        "id": c.item.id,
                        "title": c.item.title(),
                        "status": c.item.status(),
                        "assignee": c.item.meta.assignee,
                        "claimed": c.item.meta.claimed,
                        "new": c.is_new(),
                        "taken": taken(&cfg, &c.item),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let unread: Vec<_> = survey
            .unread
            .iter()
            .map(|(branch, why)| serde_json::json!({ "branch": branch, "reason": why }))
            .collect();
        let doc = serde_json::json!({ "worktrees": others, "unread": unread });
        println!("{}", serde_json::to_string_pretty(&doc)?);
        return Ok(0);
    }

    let busy: Vec<_> = survey
        .others
        .iter()
        .filter(|o| !o.copies.is_empty())
        .collect();
    if busy.is_empty() {
        eprintln!(
            "{}",
            style::dim(&match survey.others.len() {
                0 => "no other worktrees were read".to_string(),
                n => format!("{n} other worktree(s), none of which has changed the backlog"),
            })
        );
        say_unread(&survey);
        return Ok(0);
    }
    for (n, other) in busy.iter().enumerate() {
        if n > 0 {
            println!();
        }
        println!(
            "{}  {}",
            style::bold(&other.branch),
            style::dim(&other.path.display().to_string())
        );
        for copy in &other.copies {
            let item = &copy.item;
            let who = item.meta.assignee.as_deref().filter(|_| held(item));
            let mut line = format!(
                "  {}  {}",
                style::dim(&cfg.format_id(item.id)),
                paint_status(&cfg, item.status())
            );
            if let Some(who) = who {
                line.push_str(&format!("  {who}"));
            }
            line.push_str(&format!("  {}", item.title()));
            if copy.is_new() {
                line.push_str(&format!("  {}", style::dim("(new)")));
            }
            println!("{line}");
        }
    }
    let quiet = survey.others.len() - busy.len();
    if quiet > 0 {
        println!(
            "\n{}",
            style::dim(&format!(
                "{quiet} other worktree(s) have not changed the backlog"
            ))
        );
    }
    say_unread(&survey);
    Ok(0)
}

fn say_unread(survey: &Survey) {
    for (branch, why) in &survey.unread {
        eprintln!(
            "{}",
            style::dim(&format!("{branch} {why}, and was not read"))
        );
    }
}
