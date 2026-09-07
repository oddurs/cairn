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

    for def in cfg.ref_fields() {
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
/// is calling because the caller says so, and it can refuse. A command line
/// cannot: an agent with a shell runs `cairn set` and nothing here sees it.
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
                 and a person can make the change."
            ),
            Agent::Propose => bail!(
                "`{agent}` may propose `{field}` but not set it. Add a note \
                 saying what it should be and why, and a person will decide."
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
                "`{agent}` may not move an item to `{name}` directly. Add a note \
                 saying why it belongs there, and a person will decide."
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
pub struct Milestones<'a> {
    ordered: Vec<&'a Item>,
}

impl<'a> Milestones<'a> {
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
    pub fn new(cfg: &Config, items: &'a [Item]) -> Milestones<'a> {
        let Some(def) = cfg.field(MILESTONE_FIELD) else {
            return Milestones {
                ordered: Vec::new(),
            };
        };
        let target = def.target.as_deref().unwrap_or(MILESTONE_TYPE);
        let mut found: Vec<&Item> = items.iter().filter(|i| i.kind() == Some(target)).collect();

        // Depth in the dependency graph, so a milestone sorts after everything
        // it waits on however the dates read.
        let by_id: HashMap<u32, &Item> = found.iter().map(|i| (i.id, *i)).collect();
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

    /// The milestone a value names, if any.
    pub fn get(&self, key: &str) -> Option<&'a Item> {
        self.ordered
            .iter()
            .copied()
            .find(|m| m.key().is_some_and(|k| k.eq_ignore_ascii_case(key.trim())))
    }

    pub fn iter(&self) -> impl Iterator<Item = &&'a Item> {
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
