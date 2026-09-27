use crate::identity::Id;
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
// rewrites files, so it happens when you ask for it — or when the post-merge
// hook `cairn init --git` installs asks on your behalf.
//
// Every item carries a `uid` tag that a renumber never touches, and that is
// what makes the repair exact rather than a guess: at a merge, the tag finds
// each item on both sides, so a reference to the contested number can be
// traced to the side it came from and follow the item that moved.
use crate::config::Config;
use crate::item::Item;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::style;
use anyhow::{Context, Result, bail};
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
    let adopted = adopt(&cfg, &store, &mut items, args.dry_run)?;
    // Ordering decides which file keeps a contested id, so it is chosen rather
    // than incidental: oldest first, because the item that existed before the
    // collision should keep its number and the branch that arrived later should
    // move. Items with no creation date sort last, and the path breaks
    // remaining ties so two people running this on the same tree agree.
    // When the project is versioned, the repository knows something the files
    // do not: which of two items claiming an id was committed first. That is
    // the one that has been published, that other people have linked to, and
    // that must therefore keep its number.
    // Both sides of the merge being made or just made, read once: they decide
    // which item keeps a contested number, and where its references go.
    let (ours, theirs) = match merge_sides(&cfg) {
        Some((ours, theirs)) => (items_at(&cfg, ours), items_at(&cfg, theirs)),
        None => (Vec::new(), Vec::new()),
    };
    let published = arrival_side(&items, &ours, &theirs);
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
            if renamed + adopted == 0 {
                println!("{} no duplicate ids", style::green("ok:"));
            } else if renamed == 0 {
                // `adopt` has said what it did.
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
    let plan = duplicate_plan(&store, &items, &duplicates)?;

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
            style::bold(&cfg.format_id_as(*new_id, it.kind())),
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

    let moved: Vec<(Id, Id)> = plan
        .iter()
        .map(|(index, new_id)| (items[*index].id, *new_id))
        .collect();
    let arrived: Vec<bool> = plan
        .iter()
        .map(|(index, _)| published.get(&items[*index].path) == Some(&1))
        .collect();
    apply(&store, &mut items, &plan)?;
    println!("{} {} item(s)", style::green("renumbered:"), plan.len());

    // Only across a merge can a reference be traced to the side it came from.
    // Elsewhere nothing says which of two same-numbered items a reference
    // meant, so it is left on the item that kept the number, and said so.
    let traced = if arrived.iter().all(|a| *a) {
        retarget(&cfg, &mut items, &moved, &ours, &theirs)?
    } else {
        None
    };
    match traced {
        Some(0) => {}
        Some(n) => println!(
            "{} {n} item(s) that referred to what moved",
            style::green("retargeted:")
        ),
        None => eprintln!(
            "{} existing references still point at the retained items; \
             check whether any should point at the renumbered ones",
            style::yellow("note:")
        ),
    }
    Ok(0)
}

/// Number what arrived with a format-4 identity.
///
/// An item filed on a branch while the project was format 4 keeps its UUID as
/// its tag, which is exactly what it became for every item that migrated, and
/// every reference to that UUID follows it to its number.
fn adopt(cfg: &Config, store: &Store, items: &mut [Item], dry_run: bool) -> Result<usize> {
    let mut numbered: HashMap<uuid::Uuid, Id> = items
        .iter()
        .filter(|i| !i.id.is_uuid())
        .filter_map(|i| i.meta.uid.map(|u| (u, i.id)))
        .collect();
    let mut order: Vec<usize> = (0..items.len())
        .filter(|i| items[*i].id.is_uuid())
        .collect();
    order.sort_by(|a, b| {
        created_key(&items[*a])
            .cmp(&created_key(&items[*b]))
            .then_with(|| items[*a].id.cmp(&items[*b].id))
    });
    let mut changed: Vec<usize> = Vec::new();
    for index in order {
        let Id::Uuid(uid) = items[index].id else {
            continue;
        };
        if let Some(existing) = numbered.get(&uid) {
            bail!(
                "{} carries the tag of {}, which is already here — a copied file; remove one",
                store.rel(&items[index].path),
                cfg.format_id(*existing)
            );
        }
        let id = store.next_id(items)?;
        println!(
            "  {} {} {}  {}",
            style::dim(&uid.simple().to_string()[..8]),
            style::dim("->"),
            style::bold(&cfg.format_id_as(id, items[index].kind())),
            items[index].title()
        );
        numbered.insert(uid, id);
        let it = &mut items[index];
        it.id = id;
        it.meta.id = Some(id);
        it.meta.uid = Some(uid);
        cfg.remember_item(it);
        changed.push(index);
    }
    for (index, item) in items.iter_mut().enumerate() {
        let rewrote = crate::refs::edit_id_refs(cfg, item, |_, ids| {
            ids.into_iter()
                .map(|id| match id {
                    Id::Uuid(u) => numbered.get(&u).copied().unwrap_or(id),
                    Id::Num(_) => id,
                })
                .collect()
        });
        if rewrote {
            changed.push(index);
        }
    }
    changed.sort_unstable();
    changed.dedup();
    if dry_run || changed.is_empty() {
        return Ok(changed.len());
    }
    for index in &changed {
        let it = &mut items[*index];
        it.save()?;
        if let crate::store::Renamed::Blocked(taken) = store.sync_path(it)? {
            bail!(
                "cannot rename {} to {}: that name is taken",
                store.rel(&it.path),
                store.rel(&taken)
            );
        }
    }
    println!("{} {} item(s)", style::green("adopted:"), changed.len());
    Ok(changed.len())
}

/// The two sides of the merge being made or just made: the one that was here,
/// and the one that arrived. None outside a merge.
fn merge_sides(cfg: &Config) -> Option<(&'static str, &'static str)> {
    let has = |rev: &str| {
        std::process::Command::new("git")
            .args(["rev-parse", "--verify", "-q", rev])
            .current_dir(&cfg.root)
            .output()
            .is_ok_and(|o| o.status.success())
    };
    // The same order `arrival_side` asks in, for the same reason.
    if has("HEAD^2") {
        Some(("HEAD^1", "HEAD^2"))
    } else if has("MERGE_HEAD") {
        Some(("HEAD", "MERGE_HEAD"))
    } else {
        None
    }
}

/// Every item file as it was at a revision, read in one pass.
fn items_at(cfg: &Config, rev: &str) -> Vec<Item> {
    let run = |args: &[&str], input: Option<&str>| -> Option<Vec<u8>> {
        use std::io::Write;
        let mut child = std::process::Command::new("git")
            .args(args)
            .current_dir(&cfg.root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        let mut stdin = child.stdin.take()?;
        stdin.write_all(input.unwrap_or_default().as_bytes()).ok()?;
        drop(stdin);
        let out = child.wait_with_output().ok()?;
        out.status.success().then_some(out.stdout)
    };
    let dir = Store::new(cfg).rel(&cfg.items_dir());
    let Some(listing) = run(&["ls-tree", "-r", "--name-only", rev, "--", &dir], None) else {
        return Vec::new();
    };
    let paths: Vec<String> = String::from_utf8_lossy(&listing)
        .lines()
        .filter(|p| {
            std::path::Path::new(p)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        })
        .map(str::to_string)
        .collect();
    let request = paths.iter().fold(String::new(), |mut out, p| {
        use std::fmt::Write;
        let _ = writeln!(out, "{rev}:{p}");
        out
    });
    let Some(blobs) = run(&["cat-file", "--batch"], Some(&request)) else {
        return Vec::new();
    };
    let formats = cfg.id_formats();
    let mut items = Vec::new();
    let mut rest = blobs.as_slice();
    for path in &paths {
        let Some(eol) = rest.iter().position(|b| *b == b'\n') else {
            break;
        };
        let header = String::from_utf8_lossy(&rest[..eol]).into_owned();
        rest = &rest[eol + 1..];
        let Some(size) = header
            .split(' ')
            .nth(2)
            .and_then(|n| n.parse::<usize>().ok())
        else {
            continue;
        };
        let Some(body) = rest.get(..size) else { break };
        if let Ok(item) = Item::parse_with(
            &cfg.root.join(path),
            &String::from_utf8_lossy(body),
            &formats,
        ) {
            items.push(item);
        }
        rest = rest.get(size + 1..).unwrap_or_default();
    }
    items
}

/// Point references at the item that moved, where they came from its side.
///
/// A collision means the contested number was created on both sides after
/// they parted, so nothing referred to it before the branch point. A reference
/// that exists only on the arriving side meant the arriving item and follows
/// it. One that exists on both sides was added on both: it meant both items,
/// and keeps the number while gaining the new one. Items are matched across
/// sides by tag, falling back to the path for an item written before tags.
///
/// The number of items changed, or None when the sides could not be read.
fn retarget(
    cfg: &Config,
    items: &mut [Item],
    moved: &[(Id, Id)],
    ours: &[Item],
    theirs: &[Item],
) -> Result<Option<usize>> {
    if theirs.is_empty() {
        return Ok(None);
    }
    let same = |side: &'_ [Item], item: &Item| -> Option<Item> {
        side.iter()
            .find(|s| match (s.meta.uid, item.meta.uid) {
                (Some(a), Some(b)) => a == b,
                _ => s.path == item.path,
            })
            .cloned()
    };
    let names = |side: Option<&Item>, def: &crate::config::FieldDef, id: Id| {
        side.and_then(|s| crate::refs::ids_in(s, def))
            .is_some_and(|ids| ids.contains(&id))
    };
    let mut changed = 0;
    for item in items.iter_mut() {
        let (here, there) = (same(ours, item), same(theirs, item));
        let rewrote = crate::refs::edit_id_refs(cfg, item, |def, ids| {
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                out.push(id);
                for (old, new) in moved {
                    if id != *old {
                        continue;
                    }
                    let (from_ours, from_theirs) = (
                        names(here.as_ref(), def, id),
                        names(there.as_ref(), def, id),
                    );
                    match (from_ours, from_theirs) {
                        (false, true) => *out.last_mut().expect("just pushed") = *new,
                        (true, true) if crate::refs::is_many(def) => out.push(*new),
                        _ => {}
                    }
                }
            }
            out
        });
        if rewrote {
            item.save()?;
            changed += 1;
        }
    }
    Ok(Some(changed))
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
        let want = cfg.filename_for(item.id, item.kind(), item.title());
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
            // The one caller that fails on a collision, and deliberately.
            // `renumber` exists to make filenames and identifiers agree; a name
            // already taken means it cannot, and carrying on would report a
            // repair it did not make.
            if let crate::store::Renamed::Blocked(taken) = store.sync_path(item)? {
                bail!(
                    "cannot rename {} to {}: that name is taken.\n\
                     `cairn check` lists what is inconsistent; move or remove \
                     the file in the way and run this again.",
                    store.rel(&item.path),
                    store.rel(&taken)
                );
            }
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
/// Answered by tag: an item whose tag is in the published side's tree was
/// published. This used to ask `git log --follow` which commit added each file,
/// and rename detection, seeing two item files that are mostly the same
/// boilerplate, answered that a new item was an old one renamed — so both sides
/// looked published and the coin-flip came back. An item written before tags
/// is placed by its path instead.
///
/// Outside a repository, or on an ordinary commit, this is empty and the
/// previous rule applies unchanged.
fn arrival_side(items: &[Item], ours: &[Item], theirs: &[Item]) -> HashMap<PathBuf, u8> {
    let on = |side: &[Item], item: &Item| {
        side.iter().any(|s| match (s.meta.uid, item.meta.uid) {
            (Some(a), Some(b)) => a == b,
            _ => s.path == item.path,
        })
    };
    items
        .iter()
        .filter_map(|item| {
            if on(ours, item) {
                Some((item.path.clone(), 0))
            } else if on(theirs, item) {
                Some((item.path.clone(), 1))
            } else {
                None
            }
        })
        .collect()
}

/// Sort key for creation date: undated items sort after dated ones.
fn created_key(item: &Item) -> (bool, String) {
    match item.meta.created.as_deref() {
        Some(d) if !d.is_empty() => (false, d.to_string()),
        _ => (true, String::new()),
    }
}

/// Ids held by more than one file, with the indices of every item holding them.
fn duplicate_ids(items: &[Item]) -> BTreeMap<Id, Vec<usize>> {
    let mut by_id: BTreeMap<Id, Vec<usize>> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        by_id.entry(it.id).or_default().push(i);
    }
    by_id.retain(|_, v| v.len() > 1);
    by_id
}

/// The oldest item holding a duplicated id keeps it; the rest get fresh ones,
/// from the same allocator `new` uses, so a repair cannot collide with a
/// number a sibling worktree or another branch already holds.
fn duplicate_plan(
    store: &Store,
    items: &[Item],
    duplicates: &BTreeMap<Id, Vec<usize>>,
) -> Result<Vec<(usize, Id)>> {
    let mut plan = Vec::new();
    for indices in duplicates.values() {
        for index in indices.iter().skip(1) {
            plan.push((*index, store.next_id(items)?));
        }
    }
    Ok(plan)
}

/// Two phases, so a rename can never land on a file that has not moved yet.
fn apply(store: &Store, items: &mut [Item], plan: &[(usize, Id)]) -> Result<()> {
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
        it.path = store.path_for(*new_id, it.kind(), it.title());
        it.touch(&today());
        it.save()?;
        std::fs::remove_file(temp).with_context(|| format!("removing {}", temp.display()))?;
    }

    Ok(())
}
