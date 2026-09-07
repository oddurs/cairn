// cairn — an item's history, read from the repository it lives in.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See COPYING for details.
//
// The claim cairn makes is that the repository is the database. A repository
// offers one thing a database does not — history — and until this command
// existed cairn never looked at it, which left the central claim unredeemed.
//
// This is the only command that reads from git. It follows the same rule as
// everything else that touches it: git is an ordinary program cairn runs, its
// absence is a condition to explain rather than an error, and nothing here
// becomes a second source of truth. What is on disk is still the backlog; this
// only says how it got that way.
use crate::config::Config;
use crate::item::Item;
use crate::store::Store;
use crate::style;
use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(clap::Args)]
pub struct Args {
    /// Item id
    #[arg(value_name = "ID")]
    pub id: String,

    /// Show the raw diffs instead of a summary of what changed
    #[arg(long, short)]
    pub patch: bool,

    /// Show at most this many revisions, most recent first
    #[arg(long, short = 'n', value_name = "N")]
    pub max_count: Option<usize>,

    /// Machine-readable output
    #[arg(long)]
    pub json: bool,
}

/// One commit that touched the item, and what it did to it.
struct Revision {
    hash: String,
    author: String,
    /// ISO date, without the time: the frontmatter records days, so an hour
    /// here would suggest a precision the rest of the tool does not have.
    date: String,
    /// The item's path at this commit, which changes when a retitled item is
    /// renamed.
    path: String,
    changes: Vec<Change>,
}

struct Change {
    field: String,
    from: Option<String>,
    to: Option<String>,
}

impl Change {
    fn created() -> Change {
        Change {
            field: "created".into(),
            from: None,
            to: None,
        }
    }

    /// How this reads in a terminal. `created` and the body changes carry no
    /// values, so they are their own sentence; everything else is a transition.
    fn display(&self) -> String {
        match (&self.from, &self.to) {
            (None, None) => self.field.clone(),
            (from, to) => format!(
                "{} {} -> {}",
                self.field,
                style::dim(from.as_deref().unwrap_or("(unset)")),
                to.as_deref().unwrap_or("(unset)")
            ),
        }
    }
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let item = store.find(cfg.parse_id(&args.id)?)?;

    let relative = item
        .path
        .strip_prefix(&cfg.root)
        .unwrap_or(&item.path)
        .to_string_lossy()
        .replace('\\', "/");

    // Every reason there might be no history, in the order they are worth
    // reporting. None of them is a failure of the command.
    if let Some(reason) = unavailable(&cfg.root) {
        return report_nothing(&args, &item, &reason);
    }

    if args.patch {
        return raw_patches(&cfg.root, &relative, args.max_count);
    }

    let mut revisions = history(&cfg.root, &relative, item.id)?;
    if revisions.is_empty() {
        return report_nothing(
            &args,
            &item,
            "the file is not committed yet, so there is nothing to show",
        );
    }
    if let Some(n) = args.max_count
        && revisions.len() > n
    {
        revisions.drain(..revisions.len() - n);
    }

    let shallow = is_shallow(&cfg.root);
    let uncommitted = differs_from_head(&cfg.root, &relative);

    // In a shallow clone the oldest revision on hand is a horizon, not a
    // beginning, and saying "created" there is a confident falsehood.
    let mut truncated = false;
    if shallow
        && let Some(first) = revisions.first_mut()
        && shallow_boundaries(&cfg.root).contains(&first.hash)
    {
        truncated = true;
        for change in &mut first.changes {
            if change.field == "created" {
                change.field = "earliest revision in this clone".into();
            }
        }
    }

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "id": item.id,
                "available": true,
                "shallow": shallow,
                "truncated": truncated,
                "uncommitted": uncommitted,
                "revisions": revisions.iter().map(|r| json!({
                    "commit": r.hash,
                    "author": r.author,
                    "date": r.date,
                    "path": r.path,
                    "changes": r.changes.iter().map(|c| json!({
                        "field": c.field,
                        "from": c.from,
                        "to": c.to,
                    })).collect::<Vec<_>>(),
                })).collect::<Vec<_>>(),
            }))?
        );
        return Ok(0);
    }

    let width = revisions
        .iter()
        .map(|r| r.author.chars().count())
        .max()
        .unwrap_or(0);
    for rev in &revisions {
        // A commit that renamed the file but changed no field still deserves a
        // line: it is how a retitled item leaves a trace, and a gap in a
        // history is more alarming than an unexciting entry.
        let changes = if rev.changes.is_empty() {
            vec![Change {
                field: "file renamed".into(),
                from: None,
                to: None,
            }]
        } else {
            rev.changes
                .iter()
                .map(|c| Change {
                    field: c.field.clone(),
                    from: c.from.clone(),
                    to: c.to.clone(),
                })
                .collect()
        };
        for (n, change) in changes.iter().enumerate() {
            if n == 0 {
                println!(
                    "{}  {:width$}  {}",
                    style::dim(&rev.date),
                    rev.author,
                    change.display()
                );
            } else {
                // Continuation lines carry no date or author: repeating them
                // would suggest four commits where there was one.
                println!("{}  {:width$}  {}", " ".repeat(10), "", change.display());
            }
        }
    }

    if uncommitted {
        println!(
            "{}",
            style::dim("            (working tree differs from the last commit)")
        );
    }
    if truncated {
        eprintln!(
            "{}: shallow clone: anything before {} is not in this repository",
            style::yellow("note"),
            revisions.first().map(|r| r.date.as_str()).unwrap_or("here")
        );
    }
    Ok(0)
}

/// Why history cannot be read here, if it cannot.
///
/// Both cases are ordinary: cairn does not require git, and somebody may be
/// running inside a tarball. Neither is an error, so neither returns one.
fn unavailable(root: &Path) -> Option<String> {
    if Command::new("git").arg("--version").output().is_err() {
        return Some("git is not installed, and item history is read from git".into());
    }
    let inside = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !inside {
        return Some(format!(
            "{} is not in a git repository, so there is no history to read",
            root.display()
        ));
    }
    None
}

fn report_nothing(args: &Args, item: &Item, reason: &str) -> Result<i32> {
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "id": item.id,
                "available": false,
                "reason": reason,
                "revisions": [],
            }))?
        );
    } else {
        println!("{}  {}", style::bold(&item.id.to_string()), item.title());
        println!("{}", style::dim(&format!("  no history: {reason}")));
    }
    Ok(0)
}

/// The commits whose parents this clone does not have.
///
/// A shallow clone is not merely "missing old commits": the oldest revision it
/// can see of a file looks exactly like the commit that added the file, and
/// calling that "created" states something false with total confidence. If the
/// oldest revision is one of these, it is a horizon rather than a beginning.
fn shallow_boundaries(root: &Path) -> std::collections::HashSet<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(root)
        .output();
    let Ok(out) = out else {
        return Default::default();
    };
    let dir = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let dir = if dir.is_absolute() {
        dir
    } else {
        root.join(dir)
    };
    std::fs::read_to_string(dir.join("shallow"))
        .map(|s| s.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

fn is_shallow(root: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .current_dir(root)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

fn differs_from_head(root: &Path, relative: &str) -> bool {
    Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--", relative])
        .current_dir(root)
        .output()
        // A non-zero status means the file differs; a failure to run at all
        // means we cannot tell, and claiming a difference we did not observe is
        // worse than staying quiet.
        .map(|o| o.status.code() == Some(1))
        .unwrap_or(false)
}

/// Every commit that touched the file, oldest first, each with what it changed.
///
/// `--follow` is what makes this worth having: renaming an item's file when its
/// title changes is a feature, and without following renames a retitled item
/// would appear to have been created the day somebody rephrased it.
///
/// It comes with two sharp edges, both found by the tests below rather than by
/// reading the documentation.
///
/// The first is that `--follow` and `--reverse` together silently truncate the
/// history — in a repository with two items, asking for one returned exactly
/// its creating commit and nothing after. So the log is read newest-first,
/// which is `--follow`'s natural direction, and reversed here.
///
/// The second is specific to cairn and worse. Every item cairn writes has the
/// same shape, and a freshly created one is mostly boilerplate, so git's rename
/// detection cheerfully concludes that item 2 was renamed from item 1 and
/// follows into a different item's history. The correction is cairn's own data:
/// each revision says which item it is, and the trail is cut where that changes.
fn history(root: &Path, relative: &str, wanted: u32) -> Result<Vec<Revision>> {
    // \x01 separates commits and \0 separates fields, because either could
    // otherwise appear in an author name and neither can appear in one here.
    let out = Command::new("git")
        .args([
            "log",
            "--follow",
            "--name-only",
            "--format=\x01%H%x00%an%x00%ad",
            "--date=short",
            "--",
            relative,
        ])
        .current_dir(root)
        .output()
        .context("running git log")?;
    if !out.status.success() {
        // The common case is a path git has never heard of, which is not an
        // error worth stopping for.
        return Ok(Vec::new());
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut newest_first = Vec::new();
    for block in text.split('\x01').skip(1) {
        let mut lines = block.lines();
        let Some(header) = lines.next() else { continue };
        let mut parts = header.split('\0');
        let (Some(hash), Some(author), Some(date)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        // The path as it was at this commit, which is what `git show` needs.
        let path = lines
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or(relative)
            .to_string();

        newest_first.push(Revision {
            hash: hash.to_string(),
            author: author.to_string(),
            date: date.to_string(),
            path,
            changes: Vec::new(),
        });
    }

    // Read the file at each commit, newest first, stopping as soon as a
    // revision turns out to be a different item. One `git show` per revision is
    // the right trade for an item's history: a handful of revisions, and no
    // need to parse a diff to say what changed.
    let mut kept: Vec<(Revision, Option<Item>)> = Vec::new();
    for rev in newest_first {
        let parsed = at_revision(root, &rev.hash, &rev.path);
        if let Some(id) = parsed.as_ref().map(|i| i.id).or_else(|| id_in(&rev.path))
            && id != wanted
        {
            break;
        }
        kept.push((rev, parsed));
    }
    kept.reverse();

    let mut previous: Option<Item> = None;
    let mut revisions = Vec::with_capacity(kept.len());
    for (mut rev, parsed) in kept {
        if let Some(item) = parsed {
            rev.changes = match &previous {
                None => vec![Change::created()],
                Some(before) => describe(before, &item),
            };
            previous = Some(item);
        }
        revisions.push(rev);
    }
    Ok(revisions)
}

/// The id in an item's filename, for a revision too damaged to parse. The
/// filename is a rendering of the id rather than the id itself, so this is a
/// fallback and not the answer.
fn id_in(path: &str) -> Option<u32> {
    let name = path.rsplit('/').next()?;
    let digits: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn at_revision(root: &Path, hash: &str, path: &str) -> Option<Item> {
    let out = Command::new("git")
        .arg("show")
        .arg(format!("{hash}:{path}"))
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    Item::parse(Path::new(path), &text).ok()
}

/// What changed between two revisions of an item, in the vocabulary of the
/// schema rather than as a patch.
///
/// `updated` is deliberately absent: it changes on every write by definition,
/// so reporting it would put a line of noise under every real change.
fn describe(before: &Item, after: &Item) -> Vec<Change> {
    let mut out = Vec::new();

    let scalars: [(&str, Option<String>, Option<String>); 5] = [
        (
            "title",
            Some(before.title().to_string()),
            Some(after.title().to_string()),
        ),
        (
            "status",
            Some(before.status().to_string()),
            Some(after.status().to_string()),
        ),
        (
            "type",
            before.kind().map(str::to_string),
            after.kind().map(str::to_string),
        ),
        (
            "milestone",
            before.milestone().map(str::to_string),
            after.milestone().map(str::to_string),
        ),
        (
            "assignee",
            before.meta.assignee.clone(),
            after.meta.assignee.clone(),
        ),
    ];
    for (field, from, to) in scalars {
        if from != to {
            out.push(Change {
                field: field.into(),
                from,
                to,
            });
        }
    }

    for (field, from, to) in [
        (
            "labels",
            before.meta.labels.join(" "),
            after.meta.labels.join(" "),
        ),
        (
            "depends_on",
            join_ids(&before.meta.depends_on),
            join_ids(&after.meta.depends_on),
        ),
    ] {
        if from != to {
            out.push(Change {
                field: field.into(),
                from: Some(from).filter(|s| !s.is_empty()),
                to: Some(to).filter(|s| !s.is_empty()),
            });
        }
    }

    // The user's own fields. Taking the union of both sides means a field that
    // was removed is reported, not merely one that was added.
    let mut keys: Vec<String> = before
        .meta
        .extra
        .keys()
        .chain(after.meta.extra.keys())
        .filter_map(|k| k.as_str().map(str::to_string))
        .collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        if key == "updated" || key == "created" {
            continue;
        }
        let from = before.get(&key);
        let to = after.get(&key);
        if from.display() != to.display() {
            out.push(Change {
                field: key,
                from: Some(from.display()).filter(|s| !s.is_empty()),
                to: Some(to.display()).filter(|s| !s.is_empty()),
            });
        }
    }

    if before.body.trim() != after.body.trim() {
        // Appending is what `cairn note` does, and it is worth distinguishing
        // from an edit: one adds to the record, the other rewrites it.
        let appended =
            after.body.trim().starts_with(before.body.trim()) && !before.body.trim().is_empty();
        out.push(Change {
            field: if appended {
                "note added"
            } else {
                "body edited"
            }
            .into(),
            from: None,
            to: None,
        });
    }

    out
}

fn join_ids(ids: &[u32]) -> String {
    ids.iter().map(u32::to_string).collect::<Vec<_>>().join(" ")
}

/// `--patch` hands the job to git, which already formats diffs better than
/// anything reimplemented here would.
fn raw_patches(root: &Path, relative: &str, max: Option<usize>) -> Result<i32> {
    let mut cmd = Command::new("git");
    cmd.args(["log", "--follow", "--patch", "--date=short"])
        .current_dir(root);
    if let Some(n) = max {
        cmd.arg(format!("-n{n}"));
    }
    if !style::enabled() {
        cmd.arg("--no-color");
    }
    let status = cmd
        .args(["--", relative])
        .status()
        .context("running git log --patch")?;
    Ok(if status.success() { 0 } else { 1 })
}
