// cairn — the same backlog, checked out more than once.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// Agents work one item per worktree, and a claim lands in that worktree's copy
// of the item. The lock excludes the writers of one directory and nothing
// more, so a second worktree used to take the item the first had claimed a
// minute earlier: each saw only its own files.
//
// Immutable identities make the question answerable, because an id names the
// same item in every checkout. So this asks git which item files each other
// worktree has changed since it diverged from this one, and reads them. What
// it finds is derived and never stored: the record stays this checkout's, and
// filters, `list` and every write keep agreeing with it.
//
// Harrow reads the same facts the same way, in its own `worktree.rs`. The
// rules here — merge-base, working tree included, same format only — are kept
// in step with it, so the board and the claim disagree about nothing.
use crate::config::{Category, Config};
use crate::identity::Id;
use crate::item::Item;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Every other worktree of the repository a project lives in, and what each
/// has changed in the backlog.
#[derive(Default)]
pub struct Survey {
    pub others: Vec<Other>,
    /// Worktrees that could not be read as this project, and why. Said out
    /// loud, because a worktree left out looks exactly like one doing nothing.
    pub unread: Vec<(String, String)>,
}

pub struct Other {
    /// The branch, or for a detached worktree the commit, as `git worktree
    /// list` names it.
    pub branch: String,
    pub path: PathBuf,
    /// Its copies of the items it has changed or filed since it diverged.
    pub copies: Vec<Copy>,
}

pub struct Copy {
    pub item: Item,
    /// Filed there: this checkout has no item with its id.
    pub new: bool,
}

impl Survey {
    /// Look, and find nothing where there is nothing to look at.
    ///
    /// A project outside git, a machine without git, and a repository with one
    /// checkout are all ordinary, and each simply has no other worktrees. So is
    /// a project before format 4, whose counted ids name different items on
    /// different branches.
    pub fn take(cfg: &Config, ours: &[Item]) -> Survey {
        if cfg.format() < 4 || !in_repository(&cfg.root) {
            return Survey::default();
        }
        // Canonical, because git reports canonical paths and one has to be
        // found inside another. `/tmp` is `/private/tmp` on macOS.
        let Ok(root) = cfg.root.canonicalize() else {
            return Survey::default();
        };
        let Some(listing) = git(&root, &["worktree", "list", "--porcelain"]) else {
            return Survey::default();
        };
        let all = checkouts(&listing);
        // The innermost, because a worktree may be kept inside another.
        let Some(here) = all
            .iter()
            .filter(|c| root.starts_with(&c.path))
            .max_by_key(|c| c.path.components().count())
        else {
            return Survey::default();
        };
        let Ok(within) = root.strip_prefix(&here.path) else {
            return Survey::default();
        };
        let Ok(dir) = cfg
            .items_dir()
            .strip_prefix(&cfg.root)
            .map(Path::to_path_buf)
        else {
            return Survey::default();
        };
        let items = within.join(dir);
        let format = cfg.format();

        // In parallel: each is a git process that mostly waits on the disk, so
        // a reading costs the slowest worktree rather than the sum of them.
        let changed: Vec<(&Checkout, Result<Vec<PathBuf>, String>)> = std::thread::scope(|scope| {
            let handles: Vec<_> = all
                .iter()
                .filter(|c| c.path != here.path && c.path.is_dir())
                .map(|there| {
                    let (items, head) = (&items, &here.head);
                    scope.spawn(move || (there, changed(format, head, there, within, items)))
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        });

        let formats = cfg.id_formats();
        let mut unread = Vec::new();
        let mut others: Vec<Other> = changed
            .into_iter()
            .filter_map(|(there, files)| match files {
                Ok(files) => Some((there, files)),
                Err(why) => {
                    unread.push((there.name.clone(), why));
                    None
                }
            })
            .map(|(there, files)| Other {
                branch: there.name.clone(),
                path: there.path.clone(),
                copies: files
                    .iter()
                    // A copy that does not parse is mid-edit, or broken where
                    // `cairn check` in that worktree will say so. Either way
                    // it tells this checkout nothing it could act on.
                    .filter_map(|file| Item::load_with(file, &formats).ok())
                    .map(|item| Copy {
                        new: !ours.iter().any(|o| o.id == item.id),
                        item,
                    })
                    .collect(),
            })
            .collect();
        others.sort_by(|a, b| a.branch.cmp(&b.branch));
        unread.sort();
        Survey { others, unread }
    }

    /// Every other worktree's copy of one item.
    pub fn copies(&self, id: Id) -> impl Iterator<Item = (&Other, &Item)> {
        self.others.iter().flat_map(move |o| {
            o.copies
                .iter()
                .filter(move |c| c.item.id == id)
                .map(move |c| (o, &c.item))
        })
    }

    /// Where an item is being worked on, or has been finished, if anywhere
    /// other than here.
    pub fn holder(&self, cfg: &Config, id: Id) -> Option<(&Other, &Item)> {
        self.copies(id).find(|(_, copy)| taken(cfg, copy))
    }
}

/// Somebody has it there, or it has moved on from open there. A claim nobody
/// has honoured for longer than the project allows is available, as it is in
/// one directory.
pub fn taken(cfg: &Config, copy: &Item) -> bool {
    let moved = cfg.category(copy.status()) != Category::Open;
    (held(copy) || moved) && !stale(cfg, copy)
}

pub fn held(copy: &Item) -> bool {
    copy.meta.assignee.as_deref().is_some_and(|a| !a.is_empty())
}

/// `Ctx::is_stale`, for a copy this checkout's context does not hold. That
/// asks whether *our* copy is closed; here only the other copy's status says.
pub fn stale(cfg: &Config, copy: &Item) -> bool {
    let Some(after) = cfg.project.claim_stale_after else {
        return false;
    };
    !cfg.category(copy.status()).is_closed()
        && crate::filter::held_days(copy).is_some_and(|d| d >= i64::from(after))
}

/// Git's directory shared by every worktree of the repository, where a lock
/// that all of them respect can live.
pub fn common_dir(root: &Path) -> Option<PathBuf> {
    if !in_repository(root) {
        return None;
    }
    let out = git(root, &["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(out.trim_end_matches(['\r', '\n']));
    if path.as_os_str().is_empty() {
        return None;
    }
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

/// Whether there is any `.git` above the project. Checked before spawning
/// git, so a project outside a repository — every test fixture, most scratch
/// backlogs — pays nothing for a question with no answer.
fn in_repository(root: &Path) -> bool {
    root.ancestors().any(|dir| dir.join(".git").exists())
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// One entry of `git worktree list --porcelain`.
#[derive(Debug, PartialEq)]
struct Checkout {
    path: PathBuf,
    head: String,
    name: String,
}

/// Every checkout in `git worktree list --porcelain` that has files to read.
fn checkouts(porcelain: &str) -> Vec<Checkout> {
    let mut out = Vec::new();
    for block in porcelain.split("\n\n") {
        let (mut path, mut head, mut branch) = (None, None, None);
        let mut readable = true;
        for line in block.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "worktree" => path = Some(PathBuf::from(value)),
                "HEAD" => head = Some(value.to_string()),
                "branch" => {
                    branch = Some(
                        value
                            .strip_prefix("refs/heads/")
                            .unwrap_or(value)
                            .to_string(),
                    );
                }
                "bare" | "prunable" => readable = false,
                _ => {}
            }
        }
        // A worktree with no HEAD has no commit yet, and so nothing it could
        // have diverged from.
        if let (Some(path), Some(head), true) = (path, head, readable) {
            let name = branch.unwrap_or_else(|| head.chars().take(8).collect());
            out.push(Checkout {
                path: path.canonicalize().unwrap_or(path),
                head,
                name,
            });
        }
    }
    out
}

/// The item files one other worktree has changed or filed since it diverged
/// from this one, or why its ids cannot be read as ours.
fn changed(
    format: u32,
    head: &str,
    there: &Checkout,
    within: &Path,
    items: &Path,
) -> Result<Vec<PathBuf>, String> {
    // A worktree on another format may spell ids another way, so an id there
    // need not name the item it would name here.
    let config = there.path.join(within).join("cairn.toml");
    let Ok(text) = std::fs::read_to_string(&config) else {
        return Err("has no cairn.toml".into());
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return Err("has a cairn.toml that does not parse".into());
    };
    let theirs = table
        .get("format")
        .and_then(toml::Value::as_integer)
        .unwrap_or(1);
    if theirs != i64::from(format) {
        return Err(format!("is on format {theirs}, not {format}"));
    }
    let unreadable = || "could not be read by git".to_string();

    let spec = items.to_string_lossy();
    // Against the merge-base, not against our copy: a branch cut before this
    // one moved holds old copies of items it never touched, and those are not
    // work. A branch already merged here has its tip as the merge-base, so all
    // it can show is what is still uncommitted in it. Without a second commit
    // the diff reads the working tree, which is where an unmerged claim is.
    let diff = git(
        &there.path,
        &[
            "diff",
            "--name-only",
            "-z",
            "--diff-filter=AMR",
            "--merge-base",
            head,
            "--",
            &spec,
        ],
    )
    .ok_or_else(unreadable)?;
    // Filed there and not yet added: the diff cannot see those.
    let untracked = git(
        &there.path,
        &[
            "ls-files",
            "-z",
            "--others",
            "--exclude-standard",
            "--",
            &spec,
        ],
    )
    .ok_or_else(unreadable)?;

    let mut files: Vec<PathBuf> = diff
        .split('\0')
        .chain(untracked.split('\0'))
        .filter(|name| !name.is_empty())
        .filter(|name| Path::new(name).strip_prefix(items).is_ok_and(is_item_file))
        .map(|name| there.path.join(name))
        .filter(|file| file.is_file())
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

/// The rule `Store` scans by, for a path inside the items directory:
/// Markdown, not a README, and nothing hidden or reserved on the way down.
fn is_item_file(rel: &Path) -> bool {
    let Some(file) = rel.file_name().map(|n| n.to_string_lossy()) else {
        return false;
    };
    rel.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        && !file.eq_ignore_ascii_case("README.md")
        && !rel.components().any(|c| {
            let part = c.as_os_str().to_string_lossy();
            part.starts_with('_') || part.starts_with('.')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_worktree_listing_names_each_checkout_by_its_branch() {
        let listing = "worktree /r/main\nHEAD 0123456789abcdef\nbranch refs/heads/main\n\n\
                       worktree /r/agent\nHEAD fedcba9876543210\nbranch refs/heads/perf/96d2-chunks\n\n\
                       worktree /r/loose\nHEAD aaaaaaaabbbbbbbb\ndetached\n\n\
                       worktree /r/bare.git\nbare\n\n\
                       worktree /r/gone\nHEAD cccccccc\nbranch refs/heads/old\nprunable gitdir file points to non-existent location\n";
        let names: Vec<(String, String)> = checkouts(listing)
            .into_iter()
            .map(|c| (c.path.display().to_string(), c.name))
            .collect();
        assert_eq!(
            names,
            vec![
                ("/r/main".into(), "main".into()),
                ("/r/agent".into(), "perf/96d2-chunks".into()),
                ("/r/loose".into(), "aaaaaaaa".into()),
            ]
        );
    }

    #[test]
    fn only_what_the_store_would_read_counts_as_an_item() {
        assert!(is_item_file(Path::new("abc-title.md")));
        assert!(is_item_file(Path::new("sub/abc.MD")));
        assert!(!is_item_file(Path::new("README.md")));
        assert!(!is_item_file(Path::new("_legacy-ids.toml")));
        assert!(!is_item_file(Path::new("_drafts/abc.md")));
        assert!(!is_item_file(Path::new(".hidden/abc.md")));
    }
}
