// cairn — src/cmd/check.rs
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
// cairn check — validate every item against the schema. Designed for CI.
use crate::config::{Config, FieldKind};
use crate::item::Item;
use crate::render::roadmap_markdown;
use crate::store::Store;
use crate::style;
use anyhow::Result;
use clap::ArgAction;
use std::collections::{HashMap, HashSet};

#[derive(clap::Args)]
pub struct Args {
    /// Treat warnings as errors
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub strict: bool,

    /// Also verify the rendered roadmap is up to date
    #[arg(long, action = ArgAction::SetTrue)]
    pub render: bool,

    /// Print nothing when everything passes
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

pub struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    fn error(&mut self, where_: &str, msg: String) {
        self.errors.push(format!("{where_}: {msg}"));
    }
    fn warn(&mut self, where_: &str, msg: String) {
        self.warnings.push(format!("{where_}: {msg}"));
    }
    /// `file:line: message`, as the GNU Coding Standards prescribe, so editors
    /// and CI log parsers can jump straight to the offending field.
    fn error_at(&mut self, file: &str, item: &Item, key: &str, msg: String) {
        match item.line_of(key) {
            Some(line) => self.error(&format!("{file}:{line}"), msg),
            None => self.error(file, msg),
        }
    }
    fn warn_at(&mut self, file: &str, item: &Item, key: &str, msg: String) {
        match item.line_of(key) {
            Some(line) => self.warn(&format!("{file}:{line}"), msg),
            None => self.warn(file, msg),
        }
    }
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_all()?;
    let r = collect_inner(&cfg, &store, &items, args.render)?;

    // `cairn: file:line: message`, the GNU diagnostic shape.
    for w in &r.warnings {
        eprintln!("{}: {w}", style::yellow("cairn"));
    }
    for e in &r.errors {
        eprintln!("{}: {e}", style::red("cairn"));
    }

    let failed = !r.errors.is_empty() || (args.strict && !r.warnings.is_empty());
    if failed {
        eprintln!();
        eprintln!(
            "{} {} error(s), {} warning(s) across {} item(s)",
            style::red("failed:"),
            r.errors.len(),
            r.warnings.len(),
            items.len()
        );
        return Ok(1);
    }
    if !args.quiet {
        println!(
            "{} {} item(s), {} warning(s)",
            style::green("ok:"),
            items.len(),
            r.warnings.len()
        );
    }
    Ok(0)
}

/// Run every validation and return what it found. Split out from `run` so the
/// MCP server can report the same results without printing them.
pub fn collect(cfg: &Config, store: &Store) -> Result<Report> {
    let items = store.load_all()?;
    collect_inner(cfg, store, &items, false)
}

fn collect_inner(
    cfg: &Config,
    store: &Store,
    items: &[Item],
    verify_render: bool,
) -> Result<Report> {
    let mut r = Report {
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    let mut by_id: HashMap<u32, Vec<&Item>> = HashMap::new();
    for i in items {
        by_id.entry(i.id).or_default().push(i);
    }
    for (id, dupes) in &by_id {
        if dupes.len() > 1 {
            let others: Vec<String> = dupes.iter().map(|i| store.rel(&i.path)).collect();
            for it in dupes {
                let file = store.rel(&it.path);
                r.error_at(
                    &file,
                    it,
                    "id",
                    format!(
                        "id {} is used by {} files ({}) — run `cairn renumber`",
                        cfg.format_id(*id),
                        dupes.len(),
                        others.join(", ")
                    ),
                );
            }
        }
    }

    let known_ids: HashSet<u32> = items.iter().map(|i| i.id).collect();
    let declared: HashSet<&str> = cfg.fields.iter().map(|f| f.name.as_str()).collect();

    for item in items {
        let at = store.rel(&item.path);

        if item.meta.title.as_deref().unwrap_or("").trim().is_empty() {
            r.error_at(&at, item, "title", "missing `title`".into());
        }
        if item.meta.id.is_none() {
            r.warn(
                &at,
                format!(
                    "no `id:` in frontmatter (inferred {} from the filename)",
                    item.id
                ),
            );
        }

        match item.meta.status.as_deref() {
            None | Some("") => r.error(&at, "missing `status`".into()),
            Some(s) if cfg.status(s).is_none() => {
                r.error_at(&at, item, "status", format!("unknown status `{s}`"));
            }
            _ => {}
        }
        if let Some(k) = item.kind()
            && cfg.item_type(k).is_none()
        {
            r.error_at(&at, item, "type", format!("unknown type `{k}`"));
        }
        if let Some(m) = item.milestone()
            && cfg.milestone(m).is_none()
        {
            r.error_at(&at, item, "milestone", format!("unknown milestone `{m}`"));
        }

        for f in &cfg.fields {
            let v = item.get(&f.name);
            if v.is_missing() {
                if f.required {
                    r.error(&at, format!("missing required field `{}`", f.name));
                }
                continue;
            }
            match f.kind {
                FieldKind::List => {}
                _ => {
                    if let Err(e) = crate::cmd::set::validate_scalar(f, &v.display()) {
                        r.error_at(&at, item, &f.name, format!("{e}"));
                    }
                }
            }
            if f.kind == FieldKind::List
                && let crate::item::Field::List(values) = &v
                && !f.values.is_empty()
            {
                for x in values {
                    if !f
                        .values
                        .iter()
                        .any(|allowed| allowed.eq_ignore_ascii_case(x.as_str()))
                    {
                        r.error(
                            &at,
                            format!("`{}`: `{x}` is not one of {}", f.name, f.values.join(", ")),
                        );
                    }
                }
            }
        }

        for (k, _) in &item.meta.extra {
            if let serde_yaml_ng::Value::String(name) = k
                && !declared.contains(name.as_str())
            {
                r.warn_at(
                    &at,
                    item,
                    name.as_str(),
                    format!("field `{name}` is not declared in cairn.toml"),
                );
            }
        }

        for dep in &item.meta.depends_on {
            if !known_ids.contains(dep) {
                r.error_at(
                    &at,
                    item,
                    "depends_on",
                    format!("depends on {} which does not exist", cfg.format_id(*dep)),
                );
            }
        }

        let expected = cfg.filename_for(item.id, item.title());
        let actual = item
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if actual != expected {
            r.warn(
                &at,
                format!("filename does not match title (expected {expected})"),
            );
        }
    }

    for cycle in find_cycles(items) {
        let path: Vec<String> = cycle.iter().map(|id| cfg.format_id(*id)).collect();
        r.error("dependencies", format!("cycle: {}", path.join(" -> ")));
    }

    if verify_render {
        let target = cfg.root.join(&cfg.render.target);
        let current = std::fs::read_to_string(&target).unwrap_or_default();
        let expected = roadmap_markdown(cfg, store, items)?;
        if current != expected {
            r.error(
                &store.rel(&target),
                "out of date — run `cairn render`".into(),
            );
        }
    }

    Ok(r)
}

/// Depth-first search over `depends_on`, returning one representative path per
/// cycle found.
fn find_cycles(items: &[Item]) -> Vec<Vec<u32>> {
    let graph: HashMap<u32, Vec<u32>> = items
        .iter()
        .map(|i| (i.id, i.meta.depends_on.clone()))
        .collect();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut cycles = Vec::new();

    for start in graph.keys() {
        if seen.contains(start) {
            continue;
        }
        let mut stack = vec![(*start, 0usize)];
        let mut path = vec![*start];
        let mut on_path: HashSet<u32> = HashSet::from([*start]);
        while let Some((node, idx)) = stack.pop() {
            let deps = graph.get(&node).cloned().unwrap_or_default();
            if idx < deps.len() {
                stack.push((node, idx + 1));
                let next = deps[idx];
                if on_path.contains(&next) {
                    let at = path.iter().position(|n| *n == next).unwrap_or(0);
                    let mut cycle = path[at..].to_vec();
                    cycle.push(next);
                    if !cycles.contains(&cycle) {
                        cycles.push(cycle);
                    }
                } else if graph.contains_key(&next) {
                    stack.push((next, 0));
                    path.push(next);
                    on_path.insert(next);
                }
            } else {
                seen.insert(node);
                if path.last() == Some(&node) {
                    path.pop();
                    on_path.remove(&node);
                }
            }
        }
    }
    cycles
}
