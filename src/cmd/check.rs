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

    // The schema first: a configuration that misdescribes itself makes every
    // finding below suspect, and it is the cheapest thing here to check.
    schema(cfg, &mut r);

    // What a reference may resolve against: the items, plus whatever an older
    // format keeps somewhere other than the item directory.
    let universe = crate::refs::universe(cfg, items);

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
        // `created` and `updated` are reserved, so the custom-field checks below
        // never look at them — and an import from somewhere else can put
        // anything in one. A date nothing can parse sorts arbitrarily and never
        // says so.
        for (key, value) in [
            ("created", item.meta.created.as_deref()),
            ("updated", item.meta.updated.as_deref()),
        ] {
            if let Some(v) = value
                && chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_err()
            {
                r.warn_at(
                    &at,
                    item,
                    key,
                    format!("`{key}` is `{v}`, which is not a date in YYYY-MM-DD form"),
                );
            }
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

        // An item may legitimately close with a box unticked — the world moved,
        // the criterion was wrong — so this is a warning. A tool that refused
        // would teach people to write criteria they can tick rather than
        // criteria that are true.
        if cfg.project.require_criteria && cfg.category(item.status()).is_closed() {
            let c = item.criteria(cfg.project.criteria_section.as_deref());
            if c.any() && !c.complete() {
                r.warn(
                    &at,
                    format!(
                        "closed with {} of {} acceptance criteria unticked",
                        c.total - c.done,
                        c.total
                    ),
                );
            }
        }

        if let Some(problem) = crate::refs::key_problems(cfg, items)
            .into_iter()
            .find(|(id, _)| *id == item.id)
            .map(|(_, m)| m)
        {
            r.error_at(&at, item, "key", problem);
        }

        // Unbounded, but past a handful this is usually a taxonomy rather than
        // a plan. A warning says "look at this", which is all that is wanted.
        const DEEP: usize = 4;
        let depth = crate::refs::depth(items, cfg, item);
        if depth > DEEP {
            r.warn(
                &at,
                format!(
                    "{depth} levels of composition above this item; past {DEEP}, a hierarchy is usually a taxonomy rather than a plan"
                ),
            );
        }

        for def in cfg.ref_fields() {
            for value in crate::refs::values(item, def) {
                if crate::refs::resolve(&universe, def, &value).is_none() {
                    let known = crate::refs::permitted(&universe, cfg, def);
                    r.error_at(
                        &at,
                        item,
                        &def.name,
                        format!(
                            "`{}` names `{value}`, which does not exist{}",
                            def.name,
                            if known.is_empty() {
                                String::new()
                            } else {
                                format!(" (known: {})", known.join(", "))
                            }
                        ),
                    );
                } else if def.acyclic && crate::refs::would_cycle(items, def, item.id, &value) {
                    r.error_at(
                        &at,
                        item,
                        &def.name,
                        format!("`{}` = `{value}` closes a cycle", def.name),
                    );
                }
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
                // Not necessarily the title: the identifier's rendering is in
                // the filename too, so adopting `id_format` lands here.
                format!("filename does not match the item (expected {expected})"),
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

// --- the schema, checked against itself -------------------------------------

/// Validate the configuration before a single item is looked at.
///
/// `Config::validate` refuses a schema cairn cannot operate at all — a status
/// list that is empty, an enum with no values, a `target` naming no type. This
/// is the other half: a schema cairn operates perfectly and that does not mean
/// what its author thinks.
///
/// The asymmetry it closes: an item with a status the schema does not define is
/// reported at once, while a schema naming a field that does not exist was
/// nobody's problem until it quietly produced the wrong output.
///
/// Warnings rather than errors throughout, with one exception. A project must
/// not stop working because a saved view it never uses has a typo in it —
/// load-time validation is for a schema that cannot be operated, and this is
/// for one that operates and misleads. The exception is a filter that does not
/// parse, which cannot be what anybody meant.
fn schema(cfg: &Config, r: &mut Report) {
    let text =
        std::fs::read_to_string(cfg.root.join(crate::config::CONFIG_FILE)).unwrap_or_default();
    let file = crate::config::CONFIG_FILE;
    let at = |line: Option<usize>| match line {
        Some(n) => format!("{file}:{n}"),
        None => file.to_string(),
    };

    // A status that does not say what it means. Every other defaulted key in
    // the file is presentational; this one is a claim, and `open` for a status
    // somebody called `shipped` is the tool getting it exactly backwards.
    for s in &cfg.statuses {
        if s.declared_category.is_none() {
            r.warn(
                &at(block_line(&text, "status", &s.name)),
                format!(
                    "status `{}` does not declare a `category`, so it is being treated as \
                     `open` — add `category = \"open\" | \"active\" | \"done\" | \"dropped\"`",
                    s.name
                ),
            );
        }
    }

    // Anything a filter, a sort or a column may legitimately name.
    let mut known: HashSet<String> = crate::config::RESERVED_FIELDS
        .iter()
        .chain(crate::filter::DERIVED_KEYS.iter())
        .chain(Item::ALIASES.iter())
        .map(|s| (*s).to_string())
        .collect();
    known.extend(cfg.fields.iter().map(|f| f.name.clone()));
    known.extend(
        cfg.ref_fields()
            .filter_map(|f| f.inverse.clone())
            .collect::<Vec<_>>(),
    );

    let check_keys = |r: &mut Report, where_: &str, what: &str, keys: &[String]| {
        for key in keys {
            let key = key.trim().trim_start_matches('-');
            if key.is_empty() || known.contains(key) {
                continue;
            }
            r.warn(
                where_,
                format!(
                    "{what} names `{key}`, which is not a declared field — items may \
                     still carry it, but nothing in this schema says they do"
                ),
            );
        }
    };

    // `render.group_by` defaults to `milestone`, so a project that renamed the
    // field silently groups by nothing: this is the roadmap emptying itself.
    let render_at = at(key_line(&text, "render", "group_by"));
    check_keys(
        r,
        &render_at,
        "render.group_by",
        std::slice::from_ref(&cfg.render.group_by),
    );

    // `milestone` is a reserved key, so the check above accepts the name on its
    // own. The name only means something if the field is declared, and
    // `render.group_by` defaults to it — which is how renaming the field
    // silently empties the roadmap.
    if cfg.render.group_by == crate::refs::MILESTONE_FIELD
        && cfg.field(crate::refs::MILESTONE_FIELD).is_none()
    {
        r.warn(
            &render_at,
            "render.group_by is `milestone`, but no [[field]] named `milestone` is \
             declared, so the roadmap has nothing to group by"
                .to_string(),
        );
    }

    if let Some(expr) = &cfg.render.include {
        let where_ = at(key_line(&text, "render", "include"));
        match crate::filter::Filter::parse(expr) {
            Err(e) => r.error(&where_, format!("render.include does not parse: {e}")),
            Ok(f) => check_keys(
                r,
                &where_,
                "render.include",
                &f.clauses.iter().map(|c| c.key.clone()).collect::<Vec<_>>(),
            ),
        }
    }

    for (key, path) in [
        ("header", cfg.render.header.as_ref()),
        ("footer", cfg.render.footer.as_ref()),
    ] {
        if let Some(path) = path
            && !cfg.root.join(path).is_file()
        {
            r.warn(
                &at(key_line(&text, "render", key)),
                format!("render.{key} names `{path}`, which is not there"),
            );
        }
    }

    if cfg.render.link_items && cfg.project.url.is_none() {
        r.warn(
            &at(key_line(&text, "render", "link_items")),
            "render.link_items is on but project.url is not set, so nothing will be linked"
                .to_string(),
        );
    }

    for v in &cfg.views {
        let where_ = at(block_line(&text, "view", &v.name));
        if let Some(expr) = &v.filter {
            match crate::filter::Filter::parse(expr) {
                // The one error. "No items match" is true and useless: it sends
                // somebody to look at their backlog instead of at the typo.
                Err(e) => r.error(
                    &where_,
                    format!("view `{}`: filter does not parse: {e}", v.name),
                ),
                Ok(f) => check_keys(
                    r,
                    &where_,
                    &format!("view `{}` filter", v.name),
                    &f.clauses.iter().map(|c| c.key.clone()).collect::<Vec<_>>(),
                ),
            }
        }
        if let Some(sort) = &v.sort {
            let keys: Vec<String> = sort.split(',').map(str::to_string).collect();
            check_keys(r, &where_, &format!("view `{}` sort", v.name), &keys);
        }
        check_keys(
            r,
            &where_,
            &format!("view `{}` columns", v.name),
            &v.columns,
        );
        if let Some(g) = &v.group_by {
            check_keys(
                r,
                &where_,
                &format!("view `{}` group_by", v.name),
                std::slice::from_ref(g),
            );
        }
    }
}

/// The line a `[[kind]]` block naming `name` is written on.
///
/// A best-effort locator over the raw text: cairn parses the configuration with
/// serde, which does not carry spans, and a diagnostic without a line number is
/// worse than one with a slightly wrong one only in theory. When it cannot tell,
/// it says nothing and the diagnostic names the file alone.
fn block_line(text: &str, kind: &str, name: &str) -> Option<usize> {
    let header = format!("[[{kind}]]");
    let mut inside = false;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == header;
            continue;
        }
        if inside
            && let Some(rest) = t.strip_prefix("name")
            && let Some(v) = rest.trim().strip_prefix('=')
            && v.trim().trim_matches('"') == name
        {
            return Some(i + 1);
        }
    }
    None
}

/// The line `key` is set on inside `[table]`.
fn key_line(text: &str, table: &str, key: &str) -> Option<usize> {
    let header = format!("[{table}]");
    let mut inside = false;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == header;
            continue;
        }
        if inside
            && let Some(rest) = t.strip_prefix(key)
            && rest.trim_start().starts_with('=')
        {
            return Some(i + 1);
        }
    }
    None
}
