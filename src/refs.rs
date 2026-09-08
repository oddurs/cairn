// cairn — fields that name other items.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See COPYING for details.
//
// A ref is a field whose value names another item rather than describing this
// one. The difference from an enum is that the value has its own existence: a
// milestone has a due date, a reason for that date, and a history of the date
// moving, none of which a string can carry.
//
// `depends_on` was the only such field for as long as it was hardcoded, and was
// the single place where "the schema is yours" was untrue. This is the general
// version: the same machinery, declared rather than built in.
use crate::config::{Addressing, Cardinality, Config, FieldDef, FieldKind};
use crate::item::{Field, Item};
use anyhow::{Result, bail};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// The values a ref field holds on an item, as written.
pub fn values(item: &Item, def: &FieldDef) -> Vec<String> {
    match item.get(&def.name) {
        Field::List(v) => v,
        Field::Text(t) if !t.trim().is_empty() => vec![t],
        _ => Vec::new(),
    }
}

/// The item a ref value names, if anything does.
///
/// A key-addressed field resolves only by key — never by id, and never by
/// title. Accepting an id as well would make `milestone: 42` mean two things
/// depending on what happens to exist, and two spellings must not be able to
/// name different items.
pub fn resolve<'a>(items: &'a [Item], def: &FieldDef, value: &str) -> Option<&'a Item> {
    let value = value.trim();
    let of_target = |i: &&Item| match def.target.as_deref() {
        None | Some("*") => true,
        Some(t) => i.kind() == Some(t),
    };
    match def.by {
        Addressing::Key => items
            .iter()
            .filter(of_target)
            .find(|i| i.key().is_some_and(|k| k.eq_ignore_ascii_case(value))),
        Addressing::Id => {
            let n: u32 = value.trim_start_matches('#').parse().ok()?;
            items.iter().filter(of_target).find(|i| i.id == n)
        }
    }
}

/// Everything a ref field on this item points at, skipping what does not
/// resolve — `check` is what reports those.
pub fn targets<'a>(items: &'a [Item], item: &Item, def: &FieldDef) -> Vec<&'a Item> {
    values(item, def)
        .iter()
        .filter_map(|v| resolve(items, def, v))
        .collect()
}

/// What a caller may write in a ref field, for an error message.
///
/// Naming the alternatives is most of the value of the diagnostic: somebody
/// mistyping a milestone wants the list far more than the word "invalid".
pub fn permitted(items: &[Item], cfg: &Config, def: &FieldDef) -> Vec<String> {
    let mut out: Vec<String> = items
        .iter()
        .filter(|i| match def.target.as_deref() {
            None | Some("*") => true,
            Some(t) => i.kind() == Some(t),
        })
        .filter_map(|i| match def.by {
            Addressing::Key => i.key().map(str::to_string),
            Addressing::Id => Some(cfg.format_id(i.id)),
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Whether adding `value` to `item`'s ref field would close a cycle.
///
/// An item cannot be part of itself, directly or at any remove. This is the
/// same guarantee `depends_on` has always had, generalised: a project must not
/// be left in a state the tool itself rejects.
pub fn would_cycle(items: &[Item], def: &FieldDef, from: u32, value: &str) -> bool {
    let Some(target) = resolve(items, def, value) else {
        return false;
    };
    if target.id == from {
        return true;
    }
    let by_id: HashMap<u32, &Item> = items.iter().map(|i| (i.id, i)).collect();
    let mut seen = HashSet::new();
    let mut stack = vec![target.id];
    while let Some(id) = stack.pop() {
        if id == from {
            return true;
        }
        if !seen.insert(id) {
            continue;
        }
        let Some(next) = by_id.get(&id) else { continue };
        for t in targets(items, next, def) {
            stack.push(t.id);
        }
    }
    false
}

impl Config {
    /// The ref fields this project declares.
    pub fn ref_fields(&self) -> impl Iterator<Item = &FieldDef> {
        self.fields.iter().filter(|f| f.kind == FieldKind::Ref)
    }

    /// `depends_on`, described the way every other ref is described.
    ///
    /// Synthesised rather than stored: the key is documented in the
    /// specification and typed on `Meta`, so it is not a `[[field]]` a project
    /// declares. What matters is that a reader of the schema — a person or a
    /// model — meets one vocabulary rather than a general mechanism plus one
    /// special case that predates it.
    pub fn builtin_ref_fields(&self) -> Vec<FieldDef> {
        vec![FieldDef {
            name: "depends_on".into(),
            kind: FieldKind::Ref,
            target: Some("*".into()),
            cardinality: Cardinality::Many,
            by: Addressing::Id,
            acyclic: true,
            rollup: false,
            inverse: Some("blocks".into()),
            values: Vec::new(),
            required: false,
            default: None,
            description: Some("what this item is waiting on".into()),
            column: false,
            agent: crate::config::Agent::Write,
        }]
    }

    /// Whether items of this type are containers — the thing work belongs to
    /// rather than work itself.
    ///
    /// Derived rather than declared: a type named as a specific `target` is
    /// what something is filed under. `target = "*"` does not make everything a
    /// container, which is why `depends_on` does not empty `cairn next`.
    pub fn is_container(&self, kind: Option<&str>) -> bool {
        let Some(kind) = kind else { return false };
        self.ref_fields()
            .any(|f| f.target.as_deref().is_some_and(|t| t != "*" && t == kind))
    }
}

/// Whether a ref field holds several values.
pub fn is_many(def: &FieldDef) -> bool {
    def.cardinality == Cardinality::Many
}

/// Refuse a write that leaves a ref naming nothing, or closing a cycle.
///
/// Both are things `cairn check` reports, and an ordinary command must not be
/// able to create either: a project should never be left in a state the tool
/// itself rejects. That rule already applied to `depends_on`; this is it
/// applied to every ref.
pub fn validate_on_write(cfg: &Config, store: &crate::store::Store, item: &Item) -> Result<()> {
    let mut items = store.load_all()?;
    // The graph as it will be once this write lands, so a cycle is caught
    // before it exists rather than after.
    match items.iter_mut().find(|i| i.id == item.id) {
        Some(existing) => *existing = item.clone(),
        None => items.push(item.clone()),
    }

    // The built-ins as well as the declared fields. This iterated
    // `cfg.ref_fields()` alone, which is the *declared* ones — so the comment
    // above claiming the rule "already applied to `depends_on`" was false, and
    // `set 1 depends_on=999` was accepted for `check` to complain about later.
    let builtin = cfg.builtin_ref_fields();
    let declared: Vec<&FieldDef> = cfg.ref_fields().collect();
    let every = declared
        .into_iter()
        .chain(builtin.iter())
        // A project that redeclares `depends_on` gets its own definition, not
        // both.
        .fold(Vec::new(), |mut acc: Vec<&FieldDef>, f| {
            if !acc.iter().any(|x| x.name == f.name) {
                acc.push(f);
            }
            acc
        });

    for def in every {
        for value in values(item, def) {
            if resolve(&items, def, &value).is_none() {
                let known = permitted(&items, cfg, def);
                bail!(
                    "`{}` names `{value}`, which does not exist\n{}",
                    def.name,
                    if known.is_empty() {
                        "nothing to name yet".to_string()
                    } else {
                        format!("known: {}", known.join(", "))
                    }
                );
            }
            if def.acyclic && would_cycle(&items, def, item.id, &value) {
                bail!(
                    "`{}` = `{value}` would close a cycle: an item cannot be \
                     beneath itself, however far around",
                    def.name
                );
            }
        }
    }
    Ok(())
}

/// Every item whose key is not unique among items of its type, and every key
/// that could be mistaken for an identifier.
///
/// Two items answering to one key means a ref names two things, and the second
/// rule is the same one `0062` applies to identifier prefixes: `milestone: 0042`
/// must not be able to mean either a key or a number depending on what exists.
pub fn key_problems(cfg: &Config, items: &[Item]) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let mut seen: HashMap<(String, String), u32> = HashMap::new();
    for item in items {
        let Some(key) = item.key() else { continue };
        if cfg.id_format().read(key).is_ok() {
            out.push((
                item.id,
                format!(
                    "key `{key}` reads as an identifier, so a reference to it \
                     would be ambiguous"
                ),
            ));
        }
        let scope = (
            item.kind().unwrap_or_default().to_string(),
            key.to_lowercase(),
        );
        match seen.get(&scope) {
            Some(other) => out.push((
                item.id,
                format!("key `{key}` is already used by {}", cfg.format_id(*other)),
            )),
            None => {
                seen.insert(scope, item.id);
            }
        }
    }
    out
}

/// Point every reference at a key's new spelling, and report what moved.
///
/// A key is what other items call this one, so changing it is a rename. Leaving
/// the references behind would orphan them silently, which is the failure this
/// exists to prevent; `renumber` already does exactly this when an identifier
/// moves, and this is the same job one level up.
///
/// The caller holds the lock, so every item moves or none does.
pub fn rename_key(
    cfg: &Config,
    store: &crate::store::Store,
    renamed: &Item,
    old: &str,
    new: &str,
) -> Result<Vec<u32>> {
    let fields: Vec<&FieldDef> = cfg
        .ref_fields()
        .filter(|f| f.by == Addressing::Key)
        .filter(|f| match f.target.as_deref() {
            None | Some("*") => true,
            Some(t) => renamed.kind() == Some(t),
        })
        .collect();
    if fields.is_empty() {
        return Ok(Vec::new());
    }

    let mut moved = Vec::new();
    for mut item in store.load_all()? {
        if item.id == renamed.id {
            continue;
        }
        let mut touched = false;
        for def in &fields {
            let current = values(&item, def);
            if !current.iter().any(|v| v.eq_ignore_ascii_case(old)) {
                continue;
            }
            let updated: Vec<String> = current
                .into_iter()
                .map(|v| {
                    if v.eq_ignore_ascii_case(old) {
                        new.to_string()
                    } else {
                        v
                    }
                })
                .collect();
            item.set_extra(
                &def.name,
                Some(if is_many(def) {
                    Field::List(updated)
                } else {
                    Field::Text(updated.into_iter().next().unwrap_or_default())
                }),
            );
            touched = true;
        }
        if touched {
            item.save()?;
            moved.push(item.id);
        }
    }
    moved.sort_unstable();
    Ok(moved)
}

/// The longest chain of composition above an item.
///
/// Composition is unbounded on purpose: a depth limit is a decision that will
/// be wrong for somebody. But a chain much deeper than a handful usually means
/// a taxonomy where a plan was wanted, and that is a judgement cairn is
/// entitled to voice without enforcing.
pub fn depth(items: &[Item], cfg: &Config, item: &Item) -> usize {
    let composing: Vec<&FieldDef> = cfg.ref_fields().filter(|f| f.rollup).collect();
    if composing.is_empty() {
        return 0;
    }
    let by_id: HashMap<u32, &Item> = items.iter().map(|i| (i.id, i)).collect();

    // Breadth-first, tracking what has been seen, so a cycle that slipped in by
    // hand cannot make this run forever. `check` reports the cycle separately.
    let mut seen = HashSet::from([item.id]);
    let mut frontier = vec![item];
    let mut depth = 0;
    while !frontier.is_empty() {
        let mut next: Vec<&Item> = Vec::new();
        for current in frontier {
            for def in &composing {
                for t in targets(items, current, def) {
                    if seen.insert(t.id)
                        && let Some(found) = by_id.get(&t.id)
                    {
                        next.push(found);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        depth += 1;
        frontier = next;
    }
    depth
}

/// Refuse a write an agent is not permitted to make.
///
/// **A guard rail, not a boundary.** The Model Context Protocol server knows who
/// is calling — arriving on that transport is what makes a caller an agent —
/// and it refuses. A command line cannot: an agent with a shell runs
/// `cairn set` and nothing here sees it.
/// Claiming otherwise would be the first dishonest thing in these documents.
///
/// What it buys is worth having anyway. A schema that says *agents may set
/// status and add notes, and may not change priority or acceptance criteria* is
/// one somebody will let near a real backlog, and that is a larger thing than
/// any feature.
pub fn permitted_for_agent(cfg: &Config, field: &str, value: Option<&str>) -> Result<()> {
    use crate::config::Agent;
    let Some(agent) = crate::store::acting_agent() else {
        return Ok(());
    };

    if let Some(def) = cfg.field(field) {
        match def.agent {
            Agent::Write => {}
            Agent::ReadOnly => bail!(
                "`{agent}` may read `{field}` but not set it, by this project's \
                 cairn.toml. Say what you believe in a note on the item instead, \
                 or `cairn propose <ID> {field}=... --why \"...\"`, and a person \
                 can make the change."
            ),
            // Naming the command matters: the permission used to end in advice
            // to write a note, and the proposal then became prose a person had
            // to find by reading every body in the backlog.
            Agent::Propose => bail!(
                "`{agent}` may propose `{field}` but not set it:\n  \
                 cairn propose <ID> {field}={} --why \"...\"\n\
                 a person reviews it with `cairn proposals`.",
                value.unwrap_or("<value>")
            ),
        }
    }

    // Moving to a status is a separate permission from writing the field: the
    // interesting case is a project that lets an agent start work and not
    // declare it finished.
    if field == "status"
        && let Some(name) = value
        && let Some(status) = cfg.status(name)
    {
        match status.agent {
            Agent::Write => {}
            Agent::ReadOnly => bail!(
                "`{agent}` may not move an item to `{name}`, by this project's \
                 cairn.toml."
            ),
            Agent::Propose => bail!(
                "`{agent}` may not move an item to `{name}` directly:\n  \
                 cairn propose <ID> status={name} --why \"...\"\n\
                 a person reviews it with `cairn proposals`."
            ),
        }
    }
    Ok(())
}

// --- milestones, which are items -------------------------------------------

/// The type name a milestone item carries, and the field that names one.
///
/// Both are ordinary schema, declared in `cairn.toml` like anything else. These
/// constants are what `init` scaffolds and what `migrate` writes, not a
/// hardcoded meaning: a project may rename either, and everything below works
/// from the declaration rather than from the name.
pub const MILESTONE_TYPE: &str = "milestone";
pub const MILESTONE_FIELD: &str = "milestone";

/// The milestones of a project, in the order a reader should meet them.
pub struct Milestones {
    ordered: Vec<Item>,
}

impl Milestones {
    /// Gather and order the milestone items.
    ///
    /// A roadmap is a sequence, and milestones can depend on one another, so
    /// the order is that graph first: anything a milestone depends on comes
    /// before it. `due` breaks what the graph leaves free, and the identifier
    /// breaks what remains.
    ///
    /// This replaces a rule that walked the configuration backwards so an
    /// undated milestone could inherit the date of the next dated one. That
    /// existed only because milestones had no natural order. Items have one.
    pub fn new(cfg: &Config, items: &[Item]) -> Milestones {
        // A project still on format 1 keeps its milestones in the
        // configuration. Presenting them as the items they are about to become
        // is not best-effort reading: cairn knows exactly what format 1 means,
        // and refusing to show somebody their own roadmap because they have not
        // run a command yet would be the tool being difficult for its own sake.
        if cfg.format() < crate::config::CURRENT_FORMAT && !cfg.milestones.is_empty() {
            return Milestones {
                ordered: cfg
                    .milestones
                    .iter()
                    .enumerate()
                    .map(|(n, m)| as_item(cfg, n as u32 + 1, m))
                    .collect(),
            };
        }

        let Some(def) = cfg.field(MILESTONE_FIELD) else {
            return Milestones {
                ordered: Vec::new(),
            };
        };
        let target = def.target.as_deref().unwrap_or(MILESTONE_TYPE);
        let mut found: Vec<Item> = items
            .iter()
            .filter(|i| i.kind() == Some(target))
            .cloned()
            .collect();

        // Depth in the dependency graph, so a milestone sorts after everything
        // it waits on however the dates read.
        let by_id: HashMap<u32, Item> = found.iter().map(|i| (i.id, i.clone())).collect();
        let mut rank: HashMap<u32, usize> = HashMap::new();
        for m in &found {
            let mut seen = HashSet::from([m.id]);
            let mut frontier: Vec<u32> = m.meta.depends_on.clone();
            let mut depth = 0usize;
            while !frontier.is_empty() {
                let mut next = Vec::new();
                for id in frontier {
                    if !seen.insert(id) {
                        continue;
                    }
                    if let Some(dep) = by_id.get(&id) {
                        next.extend(dep.meta.depends_on.iter().copied());
                    }
                }
                if next.is_empty() {
                    break;
                }
                depth += 1;
                frontier = next;
            }
            // A milestone with dependencies sorts after one without, even when
            // the chain is a single step.
            rank.insert(
                m.id,
                if m.meta.depends_on.is_empty() {
                    0
                } else {
                    depth + 1
                },
            );
        }

        found.sort_by(|a, b| {
            rank.get(&a.id)
                .cmp(&rank.get(&b.id))
                // Undated sorts last among equals: a milestone with no date is
                // the one nobody has committed to.
                .then_with(|| match (due(a), due(b)) {
                    (Some(x), Some(y)) => x.cmp(y),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                })
                .then_with(|| a.id.cmp(&b.id))
        });
        Milestones { ordered: found }
    }

    /// The milestones as items, for resolving references against.
    pub fn as_items(&self) -> &[Item] {
        &self.ordered
    }

    /// The milestone a value names, if any.
    pub fn get(&self, key: &str) -> Option<&Item> {
        self.ordered
            .iter()
            .find(|m| m.key().is_some_and(|k| k.eq_ignore_ascii_case(key.trim())))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item> {
        self.ordered.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.ordered.is_empty()
    }

    /// The keys, in order, for a diagnostic that lists what would have worked.
    pub fn keys(&self) -> Vec<String> {
        self.ordered
            .iter()
            .filter_map(|m| m.key().map(str::to_string))
            .collect()
    }
}

/// A milestone item's due date, if it has one.
///
/// An ordinary custom field. Nothing about a date is special to cairn; it is
/// declared in the schema like `priority`, and read here because the roadmap
/// wants to sort by it.
pub fn due(item: &Item) -> Option<&str> {
    item.meta
        .extra
        .get(serde_yaml_ng::Value::String("due".into()))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// A format 1 `[[milestone]]` block, as the item it is about to become.
///
/// Not written anywhere. This exists so that reading an older project shows the
/// same roadmap the project's own cairn would have shown, rather than an empty
/// one — which is the difference between understanding an older format and
/// merely tolerating it.
fn as_item(cfg: &Config, id: u32, m: &crate::config::Milestone) -> Item {
    let title = m.title.clone().unwrap_or_else(|| m.name.clone());
    let mut item = Item {
        id,
        meta: Default::default(),
        body: String::new(),
        path: cfg.items_dir().join(cfg.filename_for(id, &title)),
        front: String::new(),
        eol: Default::default(),
    };
    item.meta.title = Some(title);
    item.meta.key = Some(m.name.clone());
    item.meta.kind = Some(MILESTONE_TYPE.to_string());
    item.meta.status = Some(
        m.status
            .clone()
            .unwrap_or_else(|| cfg.initial_status().to_string()),
    );
    if let Some(due) = &m.due {
        item.set_extra("due", Some(Field::Text(due.clone())));
    }
    if let Some(d) = &m.description {
        item.set_body(d);
    }
    // Declaration order is the order, which is what the chain the migration
    // writes will encode permanently.
    if id > 1 {
        item.meta.depends_on = vec![id - 1];
    }
    item
}

/// Everything a reference can resolve against.
///
/// The items, plus whatever an older format keeps somewhere other than the item
/// directory. On the current format this is the items and nothing else; on
/// format 1 it also includes the milestones, which live in the configuration
/// until `cairn migrate` moves them.
///
/// Without this, `cairn check` on a format 1 project reports every `milestone:`
/// as naming something that does not exist — which is true of the item
/// directory and false of the project.
pub fn universe(cfg: &Config, items: &[Item]) -> Vec<Item> {
    if cfg.format() >= crate::config::CURRENT_FORMAT {
        return items.to_vec();
    }
    let mut out = items.to_vec();
    out.extend(Milestones::new(cfg, items).as_items().iter().cloned());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CONFIG_FILE;

    /// A schema, parsed rather than constructed, so these tests exercise the
    /// same deserialisation a project does.
    fn schema(extra: &str) -> Config {
        let text = format!(
            "format = 2\n\
             [project]\nname = \"T\"\n\
             [[status]]\nname = \"todo\"\ncategory = \"open\"\n\
             [[type]]\nname = \"milestone\"\n\
             [[type]]\nname = \"epic\"\n\
             [[type]]\nname = \"feature\"\n{extra}"
        );
        let mut cfg: Config = toml::from_str(&text).expect(CONFIG_FILE);
        cfg.root = std::path::PathBuf::from("/nowhere");
        cfg
    }

    fn field(name: &str, target: &str, by: &str) -> FieldDef {
        let cfg = schema(&format!(
            "[[field]]\nname = \"{name}\"\nkind = \"ref\"\n\
             target = \"{target}\"\nby = \"{by}\"\nrollup = true\n"
        ));
        cfg.field(name).expect("the field just declared").clone()
    }

    /// An item with no file behind it. Every function here is pure, which is
    /// the whole reason to test them this way: a failure says `would_cycle` is
    /// wrong at depth 5, not that `cairn check` said something odd.
    fn item(id: u32, kind: &str, key: Option<&str>, parent: Option<&str>) -> Item {
        let mut it = Item {
            id,
            meta: Default::default(),
            body: String::new(),
            path: std::path::PathBuf::from(format!("{id:04}-x.md")),
            front: String::new(),
            eol: Default::default(),
        };
        it.meta.title = Some(format!("Item {id}"));
        it.meta.kind = Some(kind.to_string());
        it.meta.key = key.map(str::to_string);
        if let Some(p) = parent {
            it.meta.extra.insert(
                serde_yaml_ng::Value::String("parent".into()),
                serde_yaml_ng::Value::String(p.to_string()),
            );
        }
        it
    }

    // --- resolve ------------------------------------------------------------

    #[test]
    fn a_key_resolves_without_regard_to_case() {
        let def = field("parent", "milestone", "key");
        let items = vec![item(1, "milestone", Some("v0.1"), None)];
        assert_eq!(resolve(&items, &def, "V0.1").map(|i| i.id), Some(1));
        assert_eq!(resolve(&items, &def, "  v0.1  ").map(|i| i.id), Some(1));
        assert!(resolve(&items, &def, "v0.2").is_none());
    }

    /// A key-addressed field resolves only by key. Accepting an id as well
    /// would make `milestone: 42` mean two things depending on what happens to
    /// exist, and two spellings must not be able to name different items.
    #[test]
    fn a_key_addressed_field_does_not_answer_to_an_id() {
        let def = field("parent", "milestone", "key");
        let items = vec![item(42, "milestone", Some("v0.1"), None)];
        assert!(resolve(&items, &def, "42").is_none());
    }

    #[test]
    fn an_id_addressed_field_accepts_a_hash_and_nothing_else() {
        let def = field("parent", "milestone", "id");
        let items = vec![item(42, "milestone", Some("v0.1"), None)];
        assert_eq!(resolve(&items, &def, "42").map(|i| i.id), Some(42));
        assert_eq!(resolve(&items, &def, "#42").map(|i| i.id), Some(42));
        assert!(resolve(&items, &def, "v0.1").is_none());
        assert!(resolve(&items, &def, "").is_none());
    }

    /// A field naming a specific type does not resolve against another one,
    /// which is what keeps a container a container.
    #[test]
    fn a_target_type_is_honoured() {
        let def = field("parent", "milestone", "key");
        let items = vec![
            item(1, "epic", Some("v0.1"), None),
            item(2, "milestone", Some("v0.2"), None),
        ];
        assert!(resolve(&items, &def, "v0.1").is_none());
        assert_eq!(resolve(&items, &def, "v0.2").map(|i| i.id), Some(2));
    }

    // --- would_cycle --------------------------------------------------------

    #[test]
    fn an_item_cannot_be_its_own_parent() {
        let def = field("parent", "milestone", "key");
        let items = vec![item(1, "milestone", Some("a"), None)];
        assert!(would_cycle(&items, &def, 1, "a"));
    }

    #[test]
    fn a_two_item_cycle_is_refused() {
        let def = field("parent", "milestone", "key");
        // 2 already points at 1; giving 1 a parent of 2 closes the loop.
        let items = vec![
            item(1, "milestone", Some("a"), None),
            item(2, "milestone", Some("b"), Some("a")),
        ];
        assert!(would_cycle(&items, &def, 1, "b"));
    }

    #[test]
    fn a_cycle_five_deep_is_refused() {
        let def = field("parent", "milestone", "key");
        let keys = ["a", "b", "c", "d", "e"];
        let items: Vec<Item> = keys
            .iter()
            .enumerate()
            .map(|(n, k)| {
                let parent = if n == 0 { None } else { Some(keys[n - 1]) };
                item(n as u32 + 1, "milestone", Some(k), parent)
            })
            .collect();
        // e -> d -> c -> b -> a. Pointing a at e closes it.
        assert!(would_cycle(&items, &def, 1, "e"));
    }

    /// A diamond is not a cycle, and reporting one would refuse a shape people
    /// legitimately build.
    #[test]
    fn a_diamond_is_not_a_cycle() {
        let def = field("parent", "milestone", "key");
        let items = vec![
            item(1, "milestone", Some("root"), None),
            item(2, "milestone", Some("left"), Some("root")),
            item(3, "milestone", Some("right"), Some("root")),
            item(4, "milestone", Some("leaf"), Some("left")),
        ];
        assert!(!would_cycle(&items, &def, 4, "right"));
    }

    #[test]
    fn a_value_naming_nothing_closes_no_cycle() {
        let def = field("parent", "milestone", "key");
        let items = vec![item(1, "milestone", Some("a"), None)];
        assert!(!would_cycle(&items, &def, 1, "nonexistent"));
    }

    // --- depth --------------------------------------------------------------

    #[test]
    fn depth_counts_the_chain_above_an_item() {
        let cfg = schema(
            "[[field]]\nname = \"parent\"\nkind = \"ref\"\n\
             target = \"milestone\"\nby = \"key\"\nrollup = true\n",
        );
        let def = cfg.field("parent").unwrap().clone();
        let _ = &def;
        let items = vec![
            item(1, "milestone", Some("a"), None),
            item(2, "milestone", Some("b"), Some("a")),
            item(3, "milestone", Some("c"), Some("b")),
        ];
        assert_eq!(depth(&items, &cfg, &items[0]), 0, "a leaf of the chain");
        assert_eq!(depth(&items, &cfg, &items[1]), 1);
        assert_eq!(depth(&items, &cfg, &items[2]), 2);
    }

    #[test]
    fn depth_is_zero_when_nothing_composes() {
        // No `rollup`, so nothing contributes to a hierarchy.
        let cfg = schema(
            "[[field]]\nname = \"parent\"\nkind = \"ref\"\n\
             target = \"milestone\"\nby = \"key\"\n",
        );
        let items = vec![
            item(1, "milestone", Some("a"), None),
            item(2, "milestone", Some("b"), Some("a")),
        ];
        assert_eq!(depth(&items, &cfg, &items[1]), 0);
    }

    /// A cycle written by hand must not make this run forever. `check` reports
    /// the cycle; `depth` has to survive it.
    #[test]
    fn depth_terminates_on_a_cycle() {
        let cfg = schema(
            "[[field]]\nname = \"parent\"\nkind = \"ref\"\n\
             target = \"milestone\"\nby = \"key\"\nrollup = true\n",
        );
        let items = vec![
            item(1, "milestone", Some("a"), Some("b")),
            item(2, "milestone", Some("b"), Some("a")),
        ];
        assert!(depth(&items, &cfg, &items[0]) <= 2);
    }

    #[test]
    fn depth_survives_a_parent_that_is_not_there() {
        let cfg = schema(
            "[[field]]\nname = \"parent\"\nkind = \"ref\"\n\
             target = \"milestone\"\nby = \"key\"\nrollup = true\n",
        );
        let items = vec![item(1, "milestone", Some("a"), Some("gone"))];
        assert_eq!(depth(&items, &cfg, &items[0]), 0);
    }

    // --- permitted, targets, is_container -----------------------------------

    #[test]
    fn permitted_lists_only_the_target_type_and_is_sorted() {
        let cfg = schema(
            "[[field]]\nname = \"parent\"\nkind = \"ref\"\n\
             target = \"milestone\"\nby = \"key\"\n",
        );
        let def = cfg.field("parent").unwrap();
        let items = vec![
            item(1, "milestone", Some("v0.2"), None),
            item(2, "milestone", Some("v0.1"), None),
            item(3, "feature", Some("nope"), None),
            item(4, "milestone", None, None),
        ];
        assert_eq!(permitted(&items, &cfg, def), vec!["v0.1", "v0.2"]);
    }

    /// A type named as a specific target is a container. `*` does not make
    /// everything one, which is why `depends_on` does not empty `cairn next`.
    #[test]
    fn a_named_target_makes_a_container_and_a_wildcard_does_not() {
        let cfg = schema(
            "[[field]]\nname = \"parent\"\nkind = \"ref\"\n\
             target = \"milestone\"\nby = \"key\"\n\
             [[field]]\nname = \"related\"\nkind = \"ref\"\ntarget = \"*\"\nby = \"id\"\n",
        );
        assert!(cfg.is_container(Some("milestone")));
        assert!(!cfg.is_container(Some("feature")));
        assert!(!cfg.is_container(None));
    }

    // --- Milestones, which are the reason most of this exists ---------------

    #[test]
    fn no_milestone_field_means_no_milestones() {
        let cfg = schema("");
        let items = vec![item(1, "milestone", Some("v0.1"), None)];
        assert!(
            Milestones::new(&cfg, &items).is_empty(),
            "the field is what names the type; without it there is nothing to read"
        );
    }

    /// The field's own `target` decides which items are milestones, not the
    /// name of the type.
    #[test]
    fn the_milestone_field_target_is_what_is_read() {
        let cfg = schema(
            "[[field]]\nname = \"milestone\"\nkind = \"ref\"\n\
             target = \"epic\"\nby = \"key\"\nrollup = true\n",
        );
        let items = vec![
            item(1, "milestone", Some("v0.1"), None),
            item(2, "epic", Some("e1"), None),
        ];
        let found = Milestones::new(&cfg, &items);
        assert_eq!(
            found.keys(),
            vec!["e1"],
            "it read the type it was pointed at"
        );
    }

    #[test]
    fn a_keyless_milestone_is_not_offered_as_a_choice() {
        let cfg = schema(
            "[[field]]\nname = \"milestone\"\nkind = \"ref\"\n\
             target = \"milestone\"\nby = \"key\"\nrollup = true\n",
        );
        let items = vec![
            item(1, "milestone", Some("v0.1"), None),
            item(2, "milestone", None, None),
        ];
        let found = Milestones::new(&cfg, &items);
        assert_eq!(found.keys(), vec!["v0.1"]);
        assert_eq!(found.as_items().len(), 2, "but it is still a milestone");
    }

    #[test]
    fn targets_skips_what_does_not_resolve() {
        let def = field("parent", "milestone", "key");
        let mut child = item(2, "feature", None, Some("gone"));
        child.meta.extra.insert(
            serde_yaml_ng::Value::String("parent".into()),
            serde_yaml_ng::Value::String("gone".into()),
        );
        let items = vec![item(1, "milestone", Some("a"), None), child];
        assert!(targets(&items, &items[1], &def).is_empty());
    }
}
