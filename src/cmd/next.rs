use crate::identity::Id;
// cairn — finding the next thing to work on.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// The question an agent (or a person) opens the backlog with is "what should I
// do now?", and answering it from `list` requires knowing the schema, composing
// a filter, and resolving dependencies by hand. This command is that question.
use crate::cmd::{item_json, paint_status, shell_word, summary, table_fields};
use crate::config::{Category, Config};
use crate::filter::resolve;
use crate::filter::{Ctx, Filter, Op, sort_items};
use crate::item::Item;
use crate::store::Store;
use crate::style;
use crate::table::{Cell, Table};
use crate::worktree::Survey;
use anyhow::{Result, bail};
use clap::ArgAction;

#[derive(clap::Args, Clone, Default)]
pub struct Args {
    /// Show at most N items
    #[arg(short = 'n', long, value_name = "N", default_value = "5")]
    pub limit: usize,

    /// Only work assigned to WHO
    #[arg(short, long, value_name = "WHO")]
    pub assignee: Option<String>,

    /// Only work assigned to you, or to nobody
    #[arg(long, action = ArgAction::SetTrue)]
    pub mine: bool,

    /// Only work with no assignee
    #[arg(long, action = ArgAction::SetTrue)]
    pub unassigned: bool,

    /// Restrict to a milestone
    #[arg(short, long, value_name = "MILESTONE")]
    pub milestone: Option<String>,

    /// Restrict to a type
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    pub kind: Option<String>,

    /// Additional filter expression
    #[arg(short, long, value_name = "EXPR")]
    pub filter: Option<String>,

    /// Restrict candidates to a saved view (ranking stays active-first)
    #[arg(long, value_name = "NAME")]
    pub view: Option<String>,

    /// Only blocked work, each with what blocks it
    #[arg(short = 'b', long, action = ArgAction::SetTrue)]
    pub blocked: bool,

    /// Blocked work alongside ready work, as MCP `include_blocked` asks. A
    /// ranked list of both shows ready work first, so the command line asks
    /// for blocked work alone instead.
    #[arg(skip)]
    pub include_blocked: bool,

    /// Output JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,

    /// Output ids only
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
    let items = store.load_for_reading()?;
    let ctx = Ctx::new(&cfg, &items);
    let survey = Survey::take(&cfg, &items);
    let picked = select(&cfg, &ctx, &items, &survey, &args)?;

    if args.count {
        println!("{}", picked.len());
        return Ok(0);
    }
    if args.ids {
        for i in &picked {
            println!("{}", cfg.format_id(i.id));
        }
        return Ok(0);
    }
    if args.plain {
        // The same columns the table shows, as machine values rather than
        // labels — `doing`, not "in progress".
        for i in &picked {
            let mut cells = vec![cfg.format_id(i.id)];
            for f in table_fields(&cfg) {
                cells.push(resolve(i, &ctx, &f).display());
            }
            cells.push(i.status().to_string());
            cells.push(cfg.schedule_of(i).unwrap_or("").to_string());
            cells.push(
                ctx.blockers(i)
                    .iter()
                    .map(|b| cfg.format_id(*b))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            cells.push(i.title().to_string());
            println!("{}", cells.join("\t"));
        }
        return Ok(0);
    }
    if args.json {
        let arr: Vec<_> = picked
            .iter()
            .map(|i| {
                let mut v = item_json(&cfg, i, &store, false);
                if let Some(o) = v.as_object_mut() {
                    o.insert("blockers".into(), serde_json::json!(ctx.blockers(i)));
                    o.insert("ready".into(), serde_json::json!(ctx.is_ready(i)));
                }
                v
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
        return Ok(0);
    }

    let within = scope(&cfg, &ctx, &items, &args)?;
    if picked.is_empty() {
        for line in explain_empty(&cfg, &ctx, &items, &args, &within)? {
            eprintln!("{}", style::dim(&line));
        }
        say_elsewhere(&cfg, &ctx, &items, &survey);
        return Ok(0);
    }

    // Show the fields the ranking actually uses, so the order is legible rather
    // than looking arbitrary. Every optional column is offered and the empty
    // ones are dropped once the rows are known — `blocked by` is empty whenever
    // nothing is blocked, and a schema field can be empty for every item a given
    // query returns.
    let fields = table_fields(&cfg);

    let mut headers: Vec<String> = vec!["id".into()];
    headers.extend(fields.iter().cloned());
    headers.push("status".into());
    if let Some(f) = cfg.schedule_field() {
        headers.push(f.to_string());
    }
    headers.push("blocked by".into());
    headers.push("title".into());

    let refs: Vec<&str> = headers.iter().map(String::as_str).collect();
    let mut t = Table::new(&refs);
    for i in &picked {
        let id = cfg.format_id(i.id);
        let status = cfg
            .status(i.status())
            .map_or_else(|| i.status().to_string(), |s| s.display().to_string());

        let mut row = vec![Cell::styled(id.clone(), style::dim(&id))];
        for f in &fields {
            row.push(Cell::plain(resolve(i, &ctx, f).display()));
        }
        row.push(Cell::styled(&status, paint_status(&cfg, i.status())));
        if cfg.schedule_field().is_some() {
            row.push(Cell::plain(cfg.schedule_of(i).unwrap_or("")));
        }
        let blocked_by = ctx
            .blockers(i)
            .iter()
            .map(|b| cfg.format_id(*b))
            .collect::<Vec<_>>()
            .join(",");
        row.push(Cell::styled(&blocked_by, style::yellow(&blocked_by)));
        row.push(Cell::plain(i.title()));
        t.row(row);
    }
    t.drop_empty_columns(&["id", "status", "title"]);
    print!("{}", t.render());

    // A claim nobody is honouring is offered back, with who has it and for how
    // long. Offered, never taken: cairn must not quietly move work away from
    // somebody slow, so this says what it found and stops.
    let offered: Vec<&&Item> = picked.iter().filter(|i| ctx.is_stale(i)).collect();
    if !offered.is_empty() {
        println!();
        for i in &offered {
            let who = i.meta.assignee.as_deref().unwrap_or("somebody");
            let days = crate::filter::held_days(i).unwrap_or_default();
            eprintln!(
                "{} {} has been claimed by {who} for {days} day(s) — `cairn claim {}` to take it over",
                style::yellow("stale:"),
                cfg.format_id(i.id),
                cfg.format_id(i.id)
            );
        }
    }

    // "What should I do now" has a shape as well as a list — the shape of the
    // same selection, or its counts describe work the list never could.
    println!("\n{}", style::dim(&summary(&ctx, &within)));
    say_elsewhere(&cfg, &ctx, &items, &survey);
    Ok(0)
}

/// Why nothing was listed, and where to look instead. A bare "nothing is
/// ready" sends an agent away from a backlog full of work its view excluded.
fn explain_empty(
    cfg: &Config,
    ctx: &Ctx,
    items: &[Item],
    args: &Args,
    within: &[&Item],
) -> Result<Vec<String>> {
    let in_view = args
        .view
        .as_deref()
        .map_or_else(String::new, |v| format!(" in view `{v}`"));
    if args.blocked {
        return Ok(vec![format!("nothing is blocked{in_view}")]);
    }

    let mut lines = Vec::new();
    match args.view.as_deref().and_then(|v| cfg.view(v)) {
        Some(view) => {
            let expr = view
                .filter
                .as_deref()
                .map_or_else(String::new, |f| format!(" ({f})"));
            lines.push(format!("nothing is ready{in_view}{expr}"));
            let everywhere = Args {
                view: None,
                ..args.clone()
            };
            let ready = |set: &[&Item]| set.iter().filter(|i| !ctx.is_blocked(i)).count();
            let outside = ready(&scope(cfg, ctx, items, &everywhere)?) - ready(within);
            if outside > 0 {
                lines.push(format!(
                    "{outside} ready item(s) outside it — `{}` to see them",
                    rerun(&everywhere)
                ));
            }
        }
        None => lines.push("nothing is ready to start".into()),
    }

    let blocked = within.iter().filter(|i| ctx.is_blocked(i)).count();
    if blocked > 0 {
        lines.push(format!(
            "{blocked} item(s){in_view} are blocked — `{} --blocked` to see what by",
            rerun(args)
        ));
    }
    Ok(lines)
}

/// The command that asks the same question again, so a hint followed to the
/// letter stays inside the selection its count was taken over.
fn rerun(args: &Args) -> String {
    let mut cmd = String::from("cairn next");
    let mut word = |flag: &str, value: &Option<String>| {
        if let Some(v) = value {
            cmd.push_str(&format!(" {flag} {}", shell_word(v)));
        }
    };
    word("--view", &args.view);
    word("--milestone", &args.milestone);
    word("--type", &args.kind);
    word("--assignee", &args.assignee);
    if args.unassigned {
        cmd.push_str(" --unassigned");
    }
    if args.mine {
        cmd.push_str(" --mine");
    }
    if let Some(f) = &args.filter {
        cmd.push_str(&format!(" --filter {}", shell_word(f)));
    }
    cmd
}

/// What was left out because another worktree has it, said so that a short
/// list is not mistaken for a short backlog.
fn say_elsewhere(cfg: &Config, ctx: &Ctx, items: &[Item], survey: &Survey) {
    let away = items
        .iter()
        .filter(|i| !ctx.is_closed(i) && survey.holder(cfg, i.id).is_some())
        .count();
    if away > 0 {
        eprintln!(
            "{}",
            style::dim(&format!(
                "{away} item(s) under way in other worktrees are not offered — `cairn worktrees` to see them"
            ))
        );
    }
}

/// Ranked, filtered candidates. Shared with the MCP server so both surfaces
/// answer "what next?" identically.
///
/// Work another worktree holds is never a candidate. Its copy there is the
/// claim this checkout cannot see, and offering it is how two agents came to
/// build the same thing.
pub fn select<'a>(
    cfg: &Config,
    ctx: &Ctx,
    items: &'a [Item],
    survey: &Survey,
    args: &Args,
) -> Result<Vec<&'a Item>> {
    let mut chosen: Vec<Item> = matching(cfg, ctx, items, args, &criteria(cfg, args, true)?)
        .into_iter()
        .filter(|i| survey.holder(cfg, i.id).is_none())
        .filter(|i| {
            if args.blocked {
                ctx.is_blocked(i)
            } else {
                args.include_blocked || !ctx.is_blocked(i)
            }
        })
        .cloned()
        .collect();

    // Priority first if the schema has one, then milestone order, then id.
    let spec = if cfg.field("priority").is_some() {
        "priority,milestone,id"
    } else {
        "milestone,id"
    };
    sort_items(&mut chosen, spec, ctx);
    // Work already under way outranks work not yet started: the most useful
    // next action is usually finishing something. Sort is stable, so this
    // reorders the two groups without disturbing the ranking within them.
    chosen.sort_by_key(|i| cfg.category(i.status()) != Category::Active);

    let ids: Vec<Id> = chosen.iter().take(args.limit).map(|i| i.id).collect();
    Ok(ids
        .iter()
        .filter_map(|id| items.iter().find(|i| i.id == *id))
        .collect())
}

/// Open work the view and the caller's filters select, blocked or not. Every
/// count `next` prints is taken over this one set, so the summary, the hint
/// and `--blocked` cannot disagree about what "blocked" covers.
pub fn scope<'a>(cfg: &Config, ctx: &Ctx, items: &'a [Item], args: &Args) -> Result<Vec<&'a Item>> {
    // `select` has already said anything worth saying about the expressions.
    Ok(matching(
        cfg,
        ctx,
        items,
        args,
        &criteria(cfg, args, false)?,
    ))
}

/// The view and the caller's narrowing, as one filter.
fn criteria(cfg: &Config, args: &Args, warn: bool) -> Result<Filter> {
    let mut filter = saved(cfg, args.view.as_deref(), warn)?;
    if let Some(m) = &args.milestone {
        filter.push(
            cfg.schedule_field().unwrap_or("milestone"),
            Op::Eq,
            vec![m.clone()],
        );
    }
    if let Some(k) = &args.kind {
        filter.push("type", Op::Eq, vec![k.clone()]);
    }
    if let Some(a) = &args.assignee {
        filter.push("assignee", Op::Eq, vec![a.clone()]);
    }
    if args.unassigned {
        filter.push("assignee", Op::Eq, vec![String::new()]);
    }
    if let Some(expr) = &args.filter {
        filter = filter.and(if warn {
            crate::filter::parse_checked(cfg, expr, "--filter")?
        } else {
            crate::filter::parse_resolved(cfg, expr)?
        });
    }
    Ok(filter)
}

fn matching<'a>(
    cfg: &Config,
    ctx: &Ctx,
    items: &'a [Item],
    args: &Args,
    filter: &Filter,
) -> Vec<&'a Item> {
    let me = if args.mine {
        crate::store::whoami()
    } else {
        String::new()
    };
    items
        .iter()
        // Containers are what work belongs to, not work. A milestone cannot be
        // started, and offering one in answer to "what can I start" would push
        // real work off the list. This is also what stops containers that
        // depend on each other — a roadmap is a sequence — reading as blocked.
        .filter(|i| !cfg.is_container(i.kind()))
        .filter(|i| !ctx.is_closed(i))
        .filter(|i| filter.matches(i, ctx))
        // `--mine` means work nobody else has taken: assigned to me, owned by
        // me, or claimed by nobody. Owning and working are different questions
        // once an agent is doing the work, and the answer to "what is mine"
        // should include what I am answerable for.
        .filter(|i| {
            !args.mine
                || match i.meta.assignee.as_deref() {
                    None | Some("") => i
                        .meta
                        .owner
                        .as_deref()
                        .is_none_or(|o| o.eq_ignore_ascii_case(&me)),
                    Some(a) => a.eq_ignore_ascii_case(&me),
                }
        })
        .collect()
}

/// Reuse a project's selection without inventing a status or an implicit
/// default queue. Presentation settings belong to `list`, not this ranking.
pub fn view_filter(cfg: &Config, name: Option<&str>) -> Result<Filter> {
    saved(cfg, name, true)
}

fn saved(cfg: &Config, name: Option<&str>, warn: bool) -> Result<Filter> {
    let Some(name) = name else {
        return Ok(Filter::default());
    };
    let Some(view) = cfg.view(name) else {
        bail!("unknown view `{name}`; see `cairn config` for saved views");
    };
    match &view.filter {
        Some(expr) if warn => crate::filter::parse_checked(cfg, expr, &format!("view `{name}`")),
        Some(expr) => crate::filter::parse_resolved(cfg, expr),
        None => Ok(Filter::default()),
    }
}

/// What a selection view keeps from whoever is told to work from it.
pub struct Gaps {
    /// `cairn new` files work somewhere the view never looks.
    pub excludes_new: bool,
    /// Ready work outside the view, counted only when none is inside it.
    pub ready_outside: usize,
}

pub fn gaps(cfg: &Config, ctx: &Ctx, items: &[Item], name: &str) -> Result<Gaps> {
    let ready = |view: Option<String>| -> Result<usize> {
        let args = Args {
            view,
            ..Args::default()
        };
        Ok(scope(cfg, ctx, items, &args)?
            .iter()
            .filter(|i| !ctx.is_blocked(i))
            .count())
    };
    let inside = ready(Some(name.to_string()))?;
    Ok(Gaps {
        excludes_new: !saved(cfg, Some(name), false)?.admits_status(cfg, cfg.initial_status()),
        ready_outside: if inside == 0 { ready(None)? } else { 0 },
    })
}
