// cairn — reading and writing the item directory.
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
use crate::config::Config;
use crate::item::Item;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub struct Store<'a> {
    pub cfg: &'a Config,
}

impl<'a> Store<'a> {
    pub fn new(cfg: &'a Config) -> Store<'a> {
        Store { cfg }
    }

    /// Every item, ordered by id. A file that cannot be parsed is a hard error:
    /// anything that writes, and anything that produces a durable artefact,
    /// must refuse to act on a partial view of the backlog.
    pub fn load_all(&self) -> Result<Vec<Item>> {
        let (items, problems) = self.load_lenient()?;
        if let Some(first) = problems.into_iter().next() {
            return Err(first);
        }
        Ok(items)
    }

    /// Every item that parses, plus one error for each that did not.
    pub fn load_lenient(&self) -> Result<(Vec<Item>, Vec<anyhow::Error>)> {
        let dir = self.cfg.items_dir();
        if !dir.exists() {
            return Ok((Vec::new(), Vec::new()));
        }
        self.recover_staged(&dir)?;

        let mut paths = Vec::new();
        collect(&dir, &mut paths)?;
        paths.sort();
        let mut items = Vec::with_capacity(paths.len());
        let mut problems = Vec::new();
        for p in paths {
            match Item::load(&p) {
                Ok(item) => items.push(item),
                Err(e) => problems.push(e),
            }
        }
        items.sort_by_key(|i| i.id);
        Ok((items, problems))
    }

    /// For read-only commands: report what could not be read, then carry on
    /// with the rest. A backlog should not become unlistable because one file
    /// is mid-edit or was left truncated by something else.
    pub fn load_for_reading(&self) -> Result<Vec<Item>> {
        let (items, problems) = self.load_lenient()?;
        for e in &problems {
            eprintln!("{}: {e:#}", crate::style::yellow("cairn"));
        }
        if !problems.is_empty() {
            eprintln!(
                "{}",
                crate::style::dim(&format!(
                    "{} file(s) skipped; `cairn check` for details",
                    problems.len()
                ))
            );
        }
        Ok(items)
    }

    /// Put back anything a interrupted `renumber` left staged.
    ///
    /// Renumbering moves a file aside before writing it back under its new id.
    /// A crash in between leaves the staged file and no original, so the item
    /// silently disappears. Restoring it is always safe: the staged file is the
    /// last complete version, and it is only ever restored when nothing occupies
    /// its original name.
    fn recover_staged(&self, dir: &Path) -> Result<()> {
        let mut staged = Vec::new();
        collect_staged(dir, &mut staged)?;
        for path in staged {
            let original = path.with_extension("");
            if original.exists() {
                eprintln!(
                    "{} {} was left by an interrupted renumber; {} already exists, so it was \
                     not restored — move or delete it by hand",
                    crate::style::yellow("warning:"),
                    self.rel(&path),
                    self.rel(&original)
                );
                continue;
            }
            std::fs::rename(&path, &original).with_context(|| {
                format!("restoring {} to {}", path.display(), original.display())
            })?;
            eprintln!(
                "{} restored {} after an interrupted renumber",
                crate::style::yellow("note:"),
                self.rel(&original)
            );
        }
        Ok(())
    }

    pub fn find(&self, id: u32) -> Result<Item> {
        self.load_all()?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| anyhow::anyhow!("no item with id {}", self.cfg.format_id(id)))
    }

    pub fn next_id(&self, items: &[Item]) -> u32 {
        items.iter().map(|i| i.id).max().unwrap_or(0) + 1
    }

    pub fn path_for(&self, id: u32, title: &str) -> PathBuf {
        self.cfg.items_dir().join(self.cfg.filename_for(id, title))
    }

    /// Keep the filename in step with the title, preserving whatever
    /// subdirectory the file already lives in.
    pub fn sync_path(&self, item: &mut Item) -> Result<bool> {
        let want_name = self.cfg.filename_for(item.id, item.title());
        let parent = item
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.cfg.items_dir());
        let want = parent.join(&want_name);
        if want == item.path {
            return Ok(false);
        }
        if want.exists() {
            bail!("cannot rename to {}: file already exists", want.display());
        }
        std::fs::rename(&item.path, &want)
            .with_context(|| format!("renaming {} -> {}", item.path.display(), want.display()))?;
        item.path = want;
        Ok(true)
    }

    /// Path relative to the project root, for display and for links.
    pub fn rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.cfg.root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    }
}

/// Files an interrupted `renumber` moved aside.
fn collect_staged(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_staged(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "renumber") {
            out.push(path);
        }
    }
    Ok(())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "md")
            && !name.eq_ignore_ascii_case("README.md")
        {
            out.push(path);
        }
    }
    Ok(())
}

/// Write a file so that a reader sees either the old contents or the new ones,
/// never a half-written mixture.
///
/// `fs::write` truncates and then writes, so a crash, a full disk or a killed
/// process in between leaves a damaged file. Here the data goes to a temporary
/// file beside the target, is flushed to the device, and is then renamed over
/// it — rename being atomic within a filesystem on every supported platform.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    use std::io::Write;

    let parent = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    // Beside the target, so the rename stays within one filesystem, and hidden
    // and suffixed so a stray one is recognisable and is never parsed as an item.
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "cairn".to_string());
    let temp = parent.join(format!(".{name}.{}.tmp", std::process::id()));

    let write = |temp: &Path| -> Result<()> {
        let mut file =
            std::fs::File::create(temp).with_context(|| format!("creating {}", temp.display()))?;
        file.write_all(contents)
            .with_context(|| format!("writing {}", temp.display()))?;
        file.sync_all()
            .with_context(|| format!("flushing {}", temp.display()))?;
        Ok(())
    };
    if let Err(e) = write(&temp) {
        let _ = std::fs::remove_file(&temp);
        return Err(e);
    }

    if let Err(e) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(e).with_context(|| format!("replacing {}", path.display()));
    }

    // Without this the rename itself can be lost in a crash, even though the
    // file's own contents were flushed. Directories cannot be opened this way
    // on Windows, where the rename is durable regardless.
    #[cfg(unix)]
    if let Ok(dir) = std::fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// A dependency path from `from` to `to`, if one exists.
///
/// Used to refuse an edge that would close a cycle. `cairn check` already
/// reports cycles, but reporting is not enough: an ordinary command should not
/// be able to put the project into a state the tool itself calls invalid. The
/// same principle made `remove` clean up after itself.
pub fn dependency_path(items: &[Item], from: u32, to: u32) -> Option<Vec<u32>> {
    let edges: std::collections::HashMap<u32, &Vec<u32>> =
        items.iter().map(|i| (i.id, &i.meta.depends_on)).collect();

    let mut stack = vec![(from, vec![from])];
    let mut seen = std::collections::HashSet::new();
    while let Some((node, path)) = stack.pop() {
        if !seen.insert(node) {
            continue;
        }
        for next in edges.get(&node).map(|v| v.as_slice()).unwrap_or(&[]) {
            let mut extended = path.clone();
            extended.push(*next);
            if *next == to {
                return Some(extended);
            }
            stack.push((*next, extended));
        }
    }
    None
}

/// Today's date, or the one a reproducible build asked for.
///
/// `SOURCE_DATE_EPOCH` is the reproducible-builds convention: a Unix timestamp
/// that stands in for "now" so a build run twice produces the same bytes. cairn
/// honours it because the recorded demo and the website's samples embed the
/// dates of the items they create, and without this the committed recordings
/// differ from a fresh run every day at midnight — a check that fails for
/// reasons unrelated to what it guards is a check people learn to ignore.
pub fn today() -> String {
    if let Ok(raw) = std::env::var("SOURCE_DATE_EPOCH")
        && let Ok(secs) = raw.trim().parse::<i64>()
        && let Some(at) = chrono::DateTime::from_timestamp(secs, 0)
    {
        return at.format("%Y-%m-%d").to_string();
    }
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Who is acting. Explicit override first, then the repository's own idea of
/// its author, then the login name — so an agent can identify itself with
/// `CAIRN_USER=claude` without touching git configuration.
pub fn whoami() -> String {
    if let Ok(v) = std::env::var("CAIRN_USER")
        && !v.trim().is_empty()
    {
        return v.trim().to_string();
    }
    if let Ok(out) = std::process::Command::new("git")
        .args(["config", "--get", "user.name"])
        .output()
        && out.status.success()
    {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !name.is_empty() {
            return name;
        }
    }
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
