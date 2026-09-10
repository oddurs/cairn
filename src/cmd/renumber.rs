// cairn — repairing item ids.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// Sequential ids need a counter, and a counter needs coordination that a
// distributed workflow cannot provide: two branches each create "the next"
// item and both pick the same number. Git merges them cleanly — the filenames
// differ — and you are left with two items sharing an id.
//
// This command is the repair. It is deliberately not automatic: renumbering
// rewrites files, so it happens when you ask for it, not behind your back.
use crate::config::Config;
use crate::item::Item;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::style;
use anyhow::{Context, Result};
use clap::ArgAction;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Args {
    /// Show what would change without touching anything
    #[arg(short = 'n', long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Print nothing when there is nothing to do
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let _lock = Lock::acquire(&cfg)?;
    let mut items = store.load_all()?;
    // Ordering decides which file keeps a contested id, so it is chosen rather
    // than incidental: oldest first, because the item that existed before the
    // collision should keep its number and the branch that arrived later should
    // move. Items with no creation date sort last, and the path breaks
    // remaining ties so two people running this on the same tree agree.
    // When the project is versioned, the repository knows something the files
    // do not: which of two items claiming an id was committed first. That is
    // the one that has been published, that other people have linked to, and
    // that must therefore keep its number.
    let published = arrival_side(&cfg, &items);
    items.sort_by(|a, b| {
        a.id.cmp(&b.id)
            .then_with(|| {
                published
                    .get(&a.path)
                    .cmp(&published.get(&b.path))
                    // A file git has never seen sorts after one it has, so an
                    // uncommitted item never displaces a published one.
                    .then_with(|| {
                        published
                            .contains_key(&b.path)
                            .cmp(&published.contains_key(&a.path))
                    })
            })
            .then_with(|| created_key(a).cmp(&created_key(b)))
            .then_with(|| a.path.cmp(&b.path))
    });

    let duplicates = duplicate_ids(&items);
    if duplicates.is_empty() {
        // No identifier is wrong, but a filename can still disagree with one —
        // which is what happens the moment a project adopts `id_format`. Both
        // are the same job: making the identifiers on disk say what they mean.
        let renamed = rename_to_match(&cfg, &store, &mut items, args.dry_run)?;
        if !args.quiet {
            if renamed == 0 {
                println!("{} no duplicate ids", style::green("ok:"));
            } else if args.dry_run {
                println!(
                    "\n{} {renamed} file(s) would be renamed",
                    style::dim("dry run:")
                );
            } else {
                println!("{} {renamed} file(s)", style::green("renamed:"));
            }
        }
        return Ok(0);
    }
    let plan = duplicate_plan(&items, &duplicates);

    if plan.is_empty() {
        if !args.quiet {
            println!("{} nothing to renumber", style::green("ok:"));
        }
        return Ok(0);
    }

    for (index, new_id) in &plan {
        let it = &items[*index];
        println!(
            "  {} {} {}  {}",
            style::dim(&cfg.format_id(it.id)),
            style::dim("->"),
            style::bold(&cfg.format_id(*new_id)),
            it.title()
        );
    }

    if args.dry_run {
        println!(
            "\n{} {} item(s) would be renumbered",
            style::dim("dry run:"),
            plan.len()
        );
        return Ok(0);
    }

    // The retained item keeps its id, and nothing can unambiguously refer to
    // the copy, so references are left alone rather than guessed at. There was
    // a `HashMap` threaded through here for rewriting them, always empty at the
    // only call site — eighteen lines that could not run, kept alive by an
    // argument. If references ever do need rewriting, `refs::rename_key` is the
    // shape to copy, and it is tested.
    apply(&store, &mut items, &plan)?;

    println!("{} {} item(s)", style::green("renumbered:"), plan.len());
    eprintln!(
        "{} existing `depends_on` references still point at the retained items; \
         check whether any should point at the renumbered ones",
        style::yellow("note:")
    );
    Ok(0)
}

/// Bring filenames into line with the project's identifier rendering and each
/// item's title.
///
/// Adopting `id_format = "MP-{n}"` should not mean touching every item by hand,
/// and `cairn check` reports the mismatch already — this is the thing that
/// fixes what it reports.
fn rename_to_match(
    cfg: &Config,
    store: &Store,
    items: &mut [Item],
    dry_run: bool,
) -> Result<usize> {
    // The caller already holds the lock; taking it again would deadlock this
    // process against itself, which is exactly what happened the first time.
    let mut renamed = 0usize;
    for item in items.iter_mut() {
        let want = cfg.filename_for(item.id, item.title());
        let have = item
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if have == want {
            continue;
        }
        println!(
            "  {} {} {}",
            style::dim(&have),
            style::dim("->"),
            style::bold(&want)
        );
        if !dry_run {
            store.sync_path(item)?;
        }
        renamed += 1;
    }
    Ok(renamed)
}

/// Which side of a merge each item arrived from: 0 if it was already
/// published, 1 if it is arriving, absent if git cannot say.
///
/// Two branches that each allocate the same identifier look identical to cairn:
/// same creation date, distinguished only by filename, so the tie broke
/// alphabetically and got it backwards half the time. At a merge the two sides
/// are not equals — one has been published, other people have linked to it, and
/// renaming it churns history for no reason — and the repository knows which is
/// which.
///
/// The published side is HEAD during a merge, and the *first parent* of HEAD
/// once the merge has been committed, because the first parent is the branch
/// you were standing on. Those are the two moments somebody runs this, the
/// second being when the `post-merge` hook does.
///
/// Deliberately not by timestamp or by history depth: two branch commits made a
/// moment apart tie on the first, and siblings tie on the second. Both were
/// tried, and both left the original coin-flip in place.
///
/// Outside a repository, or on an ordinary commit, this is empty and the
/// previous rule applies unchanged.
fn arrival_side(cfg: &Config, items: &[Item]) -> HashMap<PathBuf, u8> {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(&cfg.root)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };

    if git(&["rev-parse", "--is-inside-work-tree"]).is_none() {
        return HashMap::new();
    }

    // Whether HEAD is a merge is asked *first*, and that order is the whole
    // trick. The `post-merge` hook runs after the merge commit exists but while
    // `.git/MERGE_HEAD` is still on disk, so testing for a merge in progress
    // first would pick HEAD — the merge commit, which contains both sides — and
    // conclude that both had been published. Which is precisely the coin-flip
    // this is meant to remove.
    let published = match git(&["rev-parse", "--verify", "-q", "HEAD^2"]) {
        // HEAD has a second parent, so it is a merge commit and its first
        // parent is the branch the merge was made on: the published side.
        Some(_) => "HEAD^1",
        // No merge commit yet. If one is in progress, HEAD is still the side
        // that was already here.
        None if git(&["rev-parse", "--verify", "-q", "MERGE_HEAD"]).is_some() => "HEAD",
        None => return HashMap::new(),
    };

    let mut side = HashMap::new();
    for item in items {
        let Ok(relative) = item.path.strip_prefix(&cfg.root) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");

        // The commit that *added* the file, not the last one to touch it: an
        // item edited yesterday can still be the one that existed first.
        let Some(added) = git(&[
            "log",
            "--diff-filter=A",
            "--follow",
            "-1",
            "--format=%H",
            "--",
            &relative,
        ])
        .filter(|s| !s.is_empty()) else {
            continue;
        };

        let already_there = std::process::Command::new("git")
            .args(["merge-base", "--is-ancestor", &added, published])
            .current_dir(&cfg.root)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        side.insert(item.path.clone(), u8::from(!already_there));
    }
    side
}

/// Sort key for creation date: undated items sort after dated ones.
fn created_key(item: &Item) -> (bool, String) {
    match item.meta.created.as_deref() {
        Some(d) if !d.is_empty() => (false, d.to_string()),
        _ => (true, String::new()),
    }
}

/// Ids held by more than one file, with the indices of every item holding them.
fn duplicate_ids(items: &[Item]) -> BTreeMap<u32, Vec<usize>> {
    let mut by_id: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        by_id.entry(it.id).or_default().push(i);
    }
    by_id.retain(|_, v| v.len() > 1);
    by_id
}

/// The oldest item holding a duplicated id keeps it; the rest get fresh ones.
fn duplicate_plan(items: &[Item], duplicates: &BTreeMap<u32, Vec<usize>>) -> Vec<(usize, u32)> {
    let mut next = items.iter().map(|i| i.id).max().unwrap_or(0) + 1;
    let mut plan = Vec::new();
    for indices in duplicates.values() {
        for index in indices.iter().skip(1) {
            plan.push((*index, next));
            next += 1;
        }
    }
    plan
}

/// Two phases, so a rename can never land on a file that has not moved yet.
fn apply(store: &Store, items: &mut [Item], plan: &[(usize, u32)]) -> Result<()> {
    let mut staged: Vec<(usize, PathBuf)> = Vec::new();
    for (index, _) in plan {
        let from = items[*index].path.clone();
        let temp = from.with_extension("md.renumber");
        std::fs::rename(&from, &temp).with_context(|| format!("staging {}", from.display()))?;
        staged.push((*index, temp));
    }

    for ((index, new_id), (_, temp)) in plan.iter().zip(staged.iter()) {
        let it = &mut items[*index];
        it.id = *new_id;
        it.meta.id = Some(*new_id);
        it.path = store.path_for(*new_id, it.title());
        it.touch(&today());
        it.save()?;
        std::fs::remove_file(temp).with_context(|| format!("removing {}", temp.display()))?;
    }

    Ok(())
}
