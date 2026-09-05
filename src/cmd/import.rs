// cairn — importing a backlog from elsewhere.
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
// Import has one hard problem: the incoming vocabulary is not this project's
// vocabulary. A GitHub issue is `open` or `closed`; this project might call
// those `icebox` and `shipped`. Guessing by name fails, so mapping happens by
// category — the one axis that is fixed across every cairn project — with
// `--map` for anything a human wants to say explicitly.
//
// The second problem is repetition. Running an import twice must not double the
// backlog, so every imported item records where it came from and matching on
// that decides update-versus-create.
use crate::cmd::export::parse_map;
use crate::cmd::set::apply;
use crate::config::{Category, Config};
use crate::interchange::Incoming;
use crate::item::Item;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::{Assign, style};
use anyhow::{Context, Result, bail};
use clap::{ArgAction, ValueEnum};
use std::collections::{HashMap, HashSet};

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Source {
    /// A cairn interchange document
    Json,
    /// GitHub issues, read through the `gh` command-line tool
    Github,
}

#[derive(clap::Args)]
pub struct Args {
    /// Where the items come from
    #[arg(long = "from", value_enum, default_value = "json")]
    pub source: Source,

    /// Input file for --from json (default: standard input)
    #[arg(value_name = "FILE")]
    pub file: Option<String>,

    /// Repository for --from github, as owner/name
    #[arg(long, value_name = "OWNER/NAME")]
    pub repo: Option<String>,

    /// Which issues to take from GitHub
    #[arg(long, value_name = "STATE", default_value = "all")]
    pub state: String,

    /// Maximum number of issues to read
    #[arg(long, value_name = "N", default_value = "200")]
    pub limit: usize,

    /// Explicit mapping: --map status:open=backlog --map type:enhancement=feature
    #[arg(long = "map", value_name = "KIND:FROM=TO")]
    pub map: Vec<String>,

    /// Milestone for everything imported that has none
    #[arg(short, long, value_name = "MILESTONE")]
    pub milestone: Option<String>,

    /// Create milestones that do not exist yet
    #[arg(long, action = ArgAction::SetTrue)]
    pub create_milestones: bool,

    /// Update items already imported from the same source
    #[arg(long, action = ArgAction::SetTrue)]
    pub update: bool,

    /// Report what would happen and write nothing
    #[arg(short = 'n', long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Print only the summary
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    // Import allocates a run of ids and writes many files; it has to be the
    // only writer for the whole of it.
    let _lock = Lock::acquire(&cfg)?;
    let existing = store.load_all()?;

    // Arguments are validated before any input is read, so a typo in --map is
    // reported even when the document turns out to be empty.
    let map = parse_map(&args.map)?;

    let (incoming, origin) = match args.source {
        Source::Json => (read_json(&cfg, &args)?, "json".to_string()),
        Source::Github => read_github(&args)?,
    };
    if incoming.is_empty() {
        eprintln!("{}", style::dim("nothing to import"));
        return Ok(0);
    }

    let by_source: HashMap<String, u32> = existing
        .iter()
        .filter_map(|i| i.meta.source.clone().map(|s| (s, i.id)))
        .collect();

    let mut created = 0usize;
    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut warnings: Vec<String> = Vec::new();
    let mut next_id = store.next_id(&existing);
    // Incoming ids are not local ids; dependencies are rewritten through this.
    let mut id_map: HashMap<u32, u32> = HashMap::new();
    let mut written: Vec<Item> = Vec::new();
    let mut new_milestones: HashSet<String> = HashSet::new();

    for inc in &incoming {
        let title = inc
            .title
            .clone()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "(untitled)".to_string());
        let source = inc
            .source
            .clone()
            .or_else(|| inc.id.map(|n| format!("{origin}#{n}")));

        // Already here?
        if let Some(source) = &source
            && let Some(local) = by_source.get(source)
        {
            if let Some(old) = inc.id {
                id_map.insert(old, *local);
            }
            if !args.update {
                skipped += 1;
                continue;
            }
            let mut item = store.find(*local)?;
            apply_fields(&mut item, &cfg, inc, &map, &args, &mut warnings, &title)?;
            item.meta.updated = inc.updated.clone().or_else(|| Some(today()));
            if !args.dry_run {
                item.save()?;
                store.sync_path(&mut item)?;
            }
            if !args.quiet {
                println!(
                    "  {} {}  {}",
                    style::yellow("update"),
                    cfg.format_id(item.id),
                    item.title()
                );
            }
            updated += 1;
            continue;
        }

        let id = next_id;
        next_id += 1;
        if let Some(old) = inc.id {
            id_map.insert(old, id);
        }
        let mut item = Item {
            id,
            meta: Default::default(),
            body: inc.body.clone().unwrap_or_default(),
            path: store.path_for(id, &title),
            front: String::new(),
            eol: Default::default(),
        };
        item.meta.title = Some(title.clone());
        item.meta.created = inc.created.clone().or_else(|| Some(today()));
        // The source's own timestamp is kept: stamping 200 imported items with
        // today's date would make "recently updated" meaningless.
        item.meta.updated = inc.updated.clone().or_else(|| Some(today()));
        item.meta.source = source;
        apply_fields(&mut item, &cfg, inc, &map, &args, &mut warnings, &title)?;

        if args.create_milestones
            && let Some(m) = &item.meta.milestone
            && cfg.milestone(m).is_none()
        {
            new_milestones.insert(m.clone());
        }
        if !args.quiet {
            println!(
                "  {} {}  {}",
                style::green("create"),
                cfg.format_id(id),
                title
            );
        }
        created += 1;
        written.push(item);
    }

    // Dependencies are rewritten once every incoming id has a local one.
    for item in written.iter_mut() {
        if let Some(inc) = incoming
            .iter()
            .find(|c| c.source.as_deref() == item.meta.source.as_deref())
            && !inc.depends_on.is_empty()
        {
            let mapped: Vec<u32> = inc
                .depends_on
                .iter()
                .filter_map(|d| id_map.get(d).copied())
                .collect();
            let lost = inc.depends_on.len() - mapped.len();
            if lost > 0 {
                warnings.push(format!(
                    "{}: dropped {lost} dependency reference(s) not present in the import",
                    cfg.format_id(item.id)
                ));
            }
            item.meta.depends_on = mapped;
        }
    }

    if args.dry_run {
        report(&warnings, created, updated, skipped, true);
        return Ok(0);
    }

    if !new_milestones.is_empty() {
        let mut names: Vec<&String> = new_milestones.iter().collect();
        names.sort();
        crate::cmd::milestone::add_many(&cfg, &names)?;
    }
    for item in &written {
        item.save()?;
    }
    report(&warnings, created, updated, skipped, false);
    Ok(0)
}

fn report(warnings: &[String], created: usize, updated: usize, skipped: usize, dry: bool) {
    for w in warnings {
        eprintln!("{} {w}", style::yellow("warning:"));
    }
    let verb = if dry { "would import:" } else { "imported:" };
    eprintln!(
        "{} {created} created, {updated} updated, {skipped} already present",
        style::green(verb)
    );
    if !dry {
        eprintln!("{}", style::dim("run `cairn check` and `cairn render`"));
    }
}

/// Map one incoming item's vocabulary onto this project's, recording anything
/// that could not be placed rather than silently dropping it.
fn apply_fields(
    item: &mut Item,
    cfg: &Config,
    inc: &Incoming,
    map: &HashMap<(String, String), String>,
    args: &Args,
    warnings: &mut Vec<String>,
    title: &str,
) -> Result<()> {
    item.meta.title = Some(title.to_string());

    let status = resolve_status(cfg, inc, map, warnings, title);
    apply(item, cfg, "status", Assign::Set(status))?;

    if let Some(kind) = &inc.kind {
        match lookup(map, "type", kind).or_else(|| cfg.item_type(kind).map(|t| t.name.clone())) {
            Some(t) if cfg.item_type(&t).is_some() => {
                apply(item, cfg, "type", Assign::Set(t))?;
            }
            _ => warnings.push(format!(
                "`{title}`: no type matches `{kind}` — left unset (use --map type:{kind}=…)"
            )),
        }
    }
    if item.kind().is_none()
        && let Some(default) = &cfg.project.default_type
    {
        apply(item, cfg, "type", Assign::Set(default.clone()))?;
    }

    let milestone = inc
        .milestone
        .as_ref()
        .and_then(|m| lookup(map, "milestone", m).or(Some(m.clone())))
        .or_else(|| args.milestone.clone());
    if let Some(m) = milestone {
        if cfg.milestone(&m).is_some() {
            apply(item, cfg, "milestone", Assign::Set(m))?;
        } else if args.create_milestones {
            // Written straight in: the milestone is created before the save.
            item.meta.milestone = Some(m);
        } else {
            warnings.push(format!(
                "`{title}`: milestone `{m}` does not exist — left unset (use --create-milestones)"
            ));
        }
    }

    if !inc.labels.is_empty() {
        let labels: Vec<String> = inc
            .labels
            .iter()
            .map(|l| lookup(map, "label", l).unwrap_or_else(|| l.clone()))
            .collect();
        apply(item, cfg, "labels", Assign::Set(labels.join(",")))?;
    }
    if let Some(a) = &inc.assignee
        && !a.is_empty()
    {
        apply(item, cfg, "assignee", Assign::Set(a.clone()))?;
    }

    // Schema defaults first, then whatever the document carried.
    for f in &cfg.fields {
        if let Some(d) = &f.default
            && item.get(&f.name).is_missing()
        {
            apply(item, cfg, &f.name, Assign::Set(d.clone()))?;
        }
    }
    for (key, value) in &inc.fields {
        let name = lookup(map, "field", key).unwrap_or_else(|| key.clone());
        if cfg.field(&name).is_none() {
            warnings.push(format!(
                "`{title}`: field `{name}` is not declared in cairn.toml — dropped"
            ));
            continue;
        }
        let text = match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Null => continue,
            other => other.to_string().trim_matches('"').to_string(),
        };
        if let Err(e) = apply(item, cfg, &name, Assign::Set(text)) {
            warnings.push(format!("`{title}`: {e}"));
        }
    }
    Ok(())
}

/// Status resolution, in order of confidence: an explicit `--map`, then an
/// exact name match, then the category — the only axis that means the same
/// thing in every cairn project.
fn resolve_status(
    cfg: &Config,
    inc: &Incoming,
    map: &HashMap<(String, String), String>,
    warnings: &mut Vec<String>,
    title: &str,
) -> String {
    if let Some(name) = &inc.status {
        if let Some(mapped) = lookup(map, "status", name)
            && cfg.status(&mapped).is_some()
        {
            return mapped;
        }
        if cfg.status(name).is_some() {
            return name.clone();
        }
    }
    let wanted = match inc.category.as_deref() {
        Some("done") | Some("closed") => Category::Done,
        Some("dropped") => Category::Dropped,
        Some("active") => Category::Active,
        Some("open") => Category::Open,
        // GitHub and friends say `closed` in the status field itself.
        _ => match inc.status.as_deref() {
            Some("closed") | Some("done") | Some("resolved") => Category::Done,
            _ => Category::Open,
        },
    };
    match cfg.statuses.iter().find(|s| s.category == wanted) {
        Some(s) => s.name.clone(),
        None => {
            warnings.push(format!(
                "`{title}`: no status with category `{}` — used `{}`",
                wanted.as_str(),
                cfg.initial_status()
            ));
            cfg.initial_status().to_string()
        }
    }
}

fn lookup(map: &HashMap<(String, String), String>, kind: &str, from: &str) -> Option<String> {
    map.get(&(kind.to_string(), from.to_lowercase())).cloned()
}

fn read_json(cfg: &Config, args: &Args) -> Result<Vec<Incoming>> {
    let text = match &args.file {
        Some(path) if path != "-" => {
            let full = cfg.root.join(path);
            std::fs::read_to_string(&full)
                .or_else(|_| std::fs::read_to_string(path))
                .with_context(|| format!("reading {path}"))?
        }
        _ => {
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
                .context("reading the document from standard input")?;
            buf
        }
    };
    let doc: serde_json::Value =
        serde_json::from_str(&text).context("parsing the interchange document")?;
    crate::interchange::items_from(&doc)
}

/// GitHub is read through `gh`, not through an HTTP client: no token handling
/// in cairn, no second place for credentials to live, and enterprise hosts work
/// because the user already configured them.
fn read_github(args: &Args) -> Result<(Vec<Incoming>, String)> {
    let Some(repo) = &args.repo else {
        bail!("--from github needs --repo owner/name");
    };
    if which("gh").is_none() {
        bail!(
            "`gh` is not installed — it is how cairn talks to GitHub\n\
             see https://cli.github.com, then run `gh auth login`"
        );
    }
    let out = std::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            &args.state,
            "--limit",
            &args.limit.to_string(),
            "--json",
            "number,title,body,state,labels,assignees,milestone,createdAt,updatedAt",
        ])
        .output()
        .context("running `gh issue list`")?;
    if !out.status.success() {
        bail!(
            "`gh issue list` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let raw: serde_json::Value =
        serde_json::from_slice(&out.stdout).context("parsing the output of `gh issue list`")?;
    let empty = vec![];
    let issues = raw.as_array().unwrap_or(&empty);

    let items = issues
        .iter()
        .map(|v| {
            let number = v
                .get("number")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as u32;
            let state = v
                .get("state")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("OPEN")
                .to_lowercase();
            Incoming {
                id: Some(number),
                title: v.get("title").and_then(|t| t.as_str()).map(str::to_string),
                status: Some(state.clone()),
                category: Some(if state == "closed" {
                    "done".into()
                } else {
                    "open".into()
                }),
                milestone: v
                    .get("milestone")
                    .and_then(|m| m.get("title"))
                    .and_then(|t| t.as_str())
                    .map(str::to_string),
                labels: v
                    .get("labels")
                    .and_then(|l| l.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
                assignee: v
                    .get("assignees")
                    .and_then(|a| a.as_array())
                    .and_then(|a| a.first())
                    .and_then(|a| a.get("login"))
                    .and_then(|l| l.as_str())
                    .map(str::to_string),
                created: v
                    .get("createdAt")
                    .and_then(|c| c.as_str())
                    .map(|c| c[..10.min(c.len())].to_string()),
                updated: v
                    .get("updatedAt")
                    .and_then(|c| c.as_str())
                    .map(|c| c[..10.min(c.len())].to_string()),
                source: Some(format!("github:{repo}#{number}")),
                body: v.get("body").and_then(|b| b.as_str()).map(str::to_string),
                ..Default::default()
            }
        })
        .collect();
    Ok((items, format!("github:{repo}")))
}

fn which(program: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|p| p.is_file())
    })
}
