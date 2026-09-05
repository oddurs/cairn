// cairn — git integration.
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
// Two branches, each adding one item, merged:
//
//     CONFLICT (content): Merge conflict in ROADMAP.md
//     cairn: id 0002 is used by 2 files — run `cairn renumber`
//
// Neither is really a conflict. The roadmap is generated, so merging its text is
// meaningless — there is a correct answer and it is "render it again". The ids
// collide because allocation reads the highest in use, which no branch can know.
//
// So nothing here invents a resolution. Both files are re-derived from the
// items, which were the only authority all along.
//
// The order matters and constrains the design. Git resolves paths in index
// order, and `ROADMAP.md` sorts before `cairn/items/…`, so when the merge driver
// runs the items it would render from have not been merged yet. The driver
// therefore resolves the conflict without pretending to know the answer — it
// keeps our side, which is a valid rendering of *something* — and the real work
// happens in the post-merge hook, once the items are settled.
use crate::config::Config;
use crate::style;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// What `.gitattributes` needs, and what the merge driver is called.
const DRIVER: &str = "cairn";

pub fn setup(cfg: &Config) -> Result<()> {
    let git_dir = git_dir(&cfg.root)?;
    let mut changed = Vec::new();

    if attributes(cfg, &mut changed)? {
        println!("{} .gitattributes", style::green("updated"));
    }
    if driver(&git_dir, &mut changed)? {
        println!("{} merge driver `{DRIVER}`", style::green("registered"));
    }
    if post_merge(&git_dir, &mut changed)? {
        println!("{} .git/hooks/post-merge", style::green("installed"));
    }

    if changed.is_empty() {
        println!("{} git integration already in place", style::dim("ok:"));
    } else {
        println!();
        println!("Merging branches that both touched the backlog will now resolve");
        println!(
            "{} and renumber colliding ids without asking.",
            cfg.render.target
        );
        println!();
        println!(
            "{}",
            style::dim(
                "The hook lives in .git/hooks, which git does not clone — run this once per \
                 working copy."
            )
        );
    }
    Ok(())
}

/// `.gitattributes` is tracked, so this part is shared by everyone.
fn attributes(cfg: &Config, changed: &mut Vec<String>) -> Result<bool> {
    let path = cfg.root.join(".gitattributes");
    let line = format!("{} merge={DRIVER}", cfg.render.target);
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if existing.lines().any(|l| l.trim() == line) {
        return Ok(false);
    }

    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&format!(
        "# {} is generated from the items. On a merge, re-derive it rather than\n\
         # trying to reconcile two renderings of different inputs.\n{line}\n",
        cfg.render.target
    ));
    crate::store::write_atomic(&path, out.as_bytes())?;
    changed.push(".gitattributes".into());
    Ok(true)
}

/// The driver itself is per-clone configuration: git deliberately refuses to
/// take executable names from a tracked file.
fn driver(git_dir: &Path, changed: &mut Vec<String>) -> Result<bool> {
    let key = format!("merge.{DRIVER}.driver");
    let already = std::process::Command::new("git")
        .args(["config", "--get", &key])
        .current_dir(git_dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if already {
        return Ok(false);
    }
    for (k, v) in [
        (
            format!("merge.{DRIVER}.name"),
            "cairn generated files".to_string(),
        ),
        (key, "cairn merge-driver %A %O %B %P".to_string()),
    ] {
        let out = std::process::Command::new("git")
            .args(["config", &k, &v])
            .current_dir(git_dir)
            .output()
            .context("running git config")?;
        if !out.status.success() {
            bail!(
                "git config {k} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
    }
    changed.push("merge driver".into());
    Ok(true)
}

const HOOK: &str = r#"#!/bin/sh
# Installed by `cairn init --git`.
#
# Ids are allocated as one more than the highest in use, which no branch can
# know, so two branches can each create the same one. Git merges them cleanly —
# the filenames differ — and leaves a project that fails `cairn check`.
#
# This runs once the merge is complete and the items are settled: renumber any
# collision, then re-derive the roadmap from what is actually there.
set -e
command -v cairn >/dev/null 2>&1 || exit 0

cairn renumber --quiet
cairn render --quiet

# A hook cannot politely amend the merge commit, so anything it changed is left
# staged for a person to look at and commit.
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "cairn: the merge changed identifiers or the roadmap; review and commit:"
  git status --short -- "$(git rev-parse --show-toplevel)" | sed "s/^/  /"
fi
"#;

fn post_merge(git_dir: &Path, changed: &mut Vec<String>) -> Result<bool> {
    let hooks = git_dir.join("hooks");
    std::fs::create_dir_all(&hooks).with_context(|| format!("creating {}", hooks.display()))?;
    let path = hooks.join("post-merge");

    if let Ok(existing) = std::fs::read_to_string(&path) {
        if existing.contains("cairn renumber") {
            return Ok(false);
        }
        // Somebody else's hook is here. Refusing is better than appending to a
        // script whose conventions we do not know.
        bail!(
            "{} already exists and is not cairn's\n\
             add these two lines to it by hand:\n  cairn renumber --quiet\n  cairn render --quiet",
            path.display()
        );
    }

    crate::store::write_atomic(&path, HOOK.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("making {} executable", path.display()))?;
    }
    changed.push("post-merge hook".into());
    Ok(true)
}

/// Resolve `.git`, following the file form a worktree or submodule uses.
fn git_dir(root: &Path) -> Result<PathBuf> {
    let dot = root.join(".git");
    if dot.is_dir() {
        return Ok(dot);
    }
    if dot.is_file() {
        // A linked worktree records `gitdir: <path>`.
        let text = std::fs::read_to_string(&dot)?;
        if let Some(rest) = text.trim().strip_prefix("gitdir:") {
            let p = PathBuf::from(rest.trim());
            return Ok(if p.is_absolute() { p } else { root.join(p) });
        }
    }
    bail!(
        "{} is not a git repository — nothing to integrate with",
        root.display()
    )
}

// --- the merge driver ------------------------------------------------------

#[derive(clap::Args)]
pub struct MergeArgs {
    /// Our version; the driver must leave the result here
    pub ours: PathBuf,
    /// The common ancestor
    pub base: PathBuf,
    /// Their version
    pub theirs: PathBuf,
    /// The path being merged, as git knows it
    pub path: Option<String>,
}

/// Resolve a conflict in a generated file.
///
/// This deliberately does almost nothing. Git resolves paths in index order and
/// the roadmap sorts before the items it is rendered from, so at this moment the
/// inputs are still unmerged and any rendering would be of the wrong thing.
/// Keeping our side is a valid rendering of a real state, and the post-merge
/// hook replaces it with the right one as soon as the items are settled.
pub fn merge_driver(args: MergeArgs) -> Result<i32> {
    let _ = (&args.base, &args.theirs);
    if !args.ours.exists() {
        bail!("merge driver: {} does not exist", args.ours.display());
    }
    eprintln!(
        "{} {} will be re-rendered after the merge",
        style::dim("cairn:"),
        args.path.as_deref().unwrap_or("the roadmap")
    );
    Ok(0)
}

/// Whether this project already has the integration, for `cairn config`.
pub fn is_configured(cfg: &Config) -> bool {
    let Ok(git_dir) = git_dir(&cfg.root) else {
        return false;
    };
    let line = format!("{} merge={DRIVER}", cfg.render.target);
    let attributes = std::fs::read_to_string(cfg.root.join(".gitattributes")).unwrap_or_default();
    let hook = git_dir.join("hooks").join("post-merge");
    attributes.lines().any(|l| l.trim() == line)
        && hook.exists()
        && std::fs::read_to_string(&hook)
            .map(|s| s.contains("cairn renumber"))
            .unwrap_or(false)
}

/// Used by `init` to explain itself when the project is not a repository yet.
pub fn in_repository(root: &Path) -> bool {
    git_dir(root).is_ok()
}
