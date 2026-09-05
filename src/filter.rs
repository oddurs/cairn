// cairn — filter expressions and sorting.
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
use crate::item::{Field, Item};
use anyhow::{Result, bail};
use std::cmp::Ordering;
use std::collections::HashSet;

/// Query context: the schema plus whatever needs knowledge of the whole item
/// set. Dependency state is the reason this exists — whether an item is blocked
/// is a property of its neighbours, not of the item itself, so it cannot be
/// answered from frontmatter alone.
pub struct Ctx<'a> {
    pub cfg: &'a Config,
    closed: HashSet<u32>,
    known: HashSet<u32>,
}

impl<'a> Ctx<'a> {
    pub fn new(cfg: &'a Config, items: &[Item]) -> Ctx<'a> {
        let mut closed = HashSet::new();
        let mut known = HashSet::new();
        for i in items {
            known.insert(i.id);
            if cfg.category(i.status()).is_closed() {
                closed.insert(i.id);
            }
        }
        Ctx { cfg, closed, known }
    }

    /// Dependencies that exist and are not finished. A reference to an item
    /// that does not exist is a `cairn check` error, not a blocker — refusing
    /// to surface work because of a typo elsewhere would be worse than the typo.
    pub fn blockers(&self, item: &Item) -> Vec<u32> {
        item.meta
            .depends_on
            .iter()
            .copied()
            .filter(|d| self.known.contains(d) && !self.closed.contains(d))
            .collect()
    }

    pub fn is_blocked(&self, item: &Item) -> bool {
        !self.blockers(item).is_empty()
    }

    pub fn is_closed(&self, item: &Item) -> bool {
        self.cfg.category(item.status()).is_closed()
    }

    /// Work that could be started right now: not finished, nothing in the way.
    pub fn is_ready(&self, item: &Item) -> bool {
        !self.is_closed(item) && !self.is_blocked(item)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Eq,
    Ne,
    Contains,
    NotContains,
    Gt,
    Ge,
    Lt,
    Le,
}

#[derive(Debug, Clone)]
pub struct Clause {
    pub key: String,
    pub op: Op,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub clauses: Vec<Clause>,
}

impl Filter {
    pub fn parse(expr: &str) -> Result<Filter> {
        let mut clauses = Vec::new();
        for raw in expr.split(',') {
            let part = raw.trim();
            if part.is_empty() {
                continue;
            }
            clauses.push(Clause::parse(part)?);
        }
        Ok(Filter { clauses })
    }

    pub fn and(mut self, other: Filter) -> Filter {
        self.clauses.extend(other.clauses);
        self
    }

    pub fn push(&mut self, key: &str, op: Op, values: Vec<String>) {
        self.clauses.push(Clause {
            key: key.to_string(),
            op,
            values,
        });
    }

    pub fn matches(&self, item: &Item, ctx: &Ctx) -> bool {
        self.clauses.iter().all(|c| c.matches(item, ctx))
    }
}

// Longest operators first so `!=` is not read as `!` + `=`.
const OPS: &[(&str, Op)] = &[
    ("!~", Op::NotContains),
    ("!=", Op::Ne),
    (">=", Op::Ge),
    ("<=", Op::Le),
    ("==", Op::Eq),
    ("~", Op::Contains),
    ("=", Op::Eq),
    (">", Op::Gt),
    ("<", Op::Lt),
];

impl Clause {
    pub fn parse(part: &str) -> Result<Clause> {
        for (token, op) in OPS {
            if let Some(pos) = part.find(token) {
                let key = part[..pos].trim();
                let value = part[pos + token.len()..].trim();
                if key.is_empty() {
                    bail!("filter `{part}`: missing field name before `{token}`");
                }
                let values: Vec<String> = if value.is_empty() {
                    vec![String::new()]
                } else {
                    value.split('|').map(|v| v.trim().to_string()).collect()
                };
                return Ok(Clause {
                    key: key.to_string(),
                    op: *op,
                    values,
                });
            }
        }
        bail!("filter `{part}`: expected `field=value` (operators: = != ~ !~ > >= < <=)")
    }

    fn matches(&self, item: &Item, ctx: &Ctx) -> bool {
        let field = resolve(item, ctx, &self.key);
        match self.op {
            Op::Eq => self.any(|v| eq_field(&field, v, &self.key)),
            Op::Ne => !self.any(|v| eq_field(&field, v, &self.key)),
            Op::Contains => self.any(|v| contains_field(&field, v)),
            Op::NotContains => !self.any(|v| contains_field(&field, v)),
            Op::Gt | Op::Ge | Op::Lt | Op::Le => {
                let lhs = field.display();
                self.any(|v| match compare(&lhs, v) {
                    Ordering::Less => matches!(self.op, Op::Lt | Op::Le),
                    Ordering::Equal => matches!(self.op, Op::Ge | Op::Le),
                    Ordering::Greater => matches!(self.op, Op::Gt | Op::Ge),
                })
            }
        }
    }

    fn any(&self, mut pred: impl FnMut(&str) -> bool) -> bool {
        self.values.iter().any(|v| pred(v))
    }
}

/// Field lookup with the config-aware pseudo-fields layered on top.
pub fn resolve(item: &Item, ctx: &Ctx, key: &str) -> Field {
    match key {
        "category" => Field::Text(ctx.cfg.category(item.status()).as_str().to_string()),
        "closed" | "done" => Field::Text(ctx.is_closed(item).to_string()),
        "blocked" => Field::Text(ctx.is_blocked(item).to_string()),
        "ready" => Field::Text(ctx.is_ready(item).to_string()),
        "blockers" => Field::List(ctx.blockers(item).iter().map(u32::to_string).collect()),
        _ => item.get(key),
    }
}

fn eq_field(field: &Field, needle: &str, key: &str) -> bool {
    if needle.is_empty() {
        return field.is_missing();
    }
    // Ids compare numerically so `id=12` finds `0012`.
    if key == "id"
        && let (Ok(a), Ok(b)) = (field.display().parse::<u32>(), needle.parse::<u32>())
    {
        return a == b;
    }
    field
        .values()
        .iter()
        .any(|v| v.eq_ignore_ascii_case(needle))
}

fn contains_field(field: &Field, needle: &str) -> bool {
    if needle.is_empty() {
        return !field.is_missing();
    }
    let n = needle.to_lowercase();
    field.values().iter().any(|v| v.to_lowercase().contains(&n))
}

fn compare(lhs: &str, rhs: &str) -> Ordering {
    match (lhs.parse::<f64>(), rhs.parse::<f64>()) {
        (Ok(a), Ok(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
        _ => lhs.cmp(rhs),
    }
}

/// Sort in place. `spec` is a comma-separated list of keys, each optionally
/// prefixed with `-` for descending: `-priority,status,id`.
pub fn sort_items(items: &mut [Item], spec: &str, ctx: &Ctx) {
    let keys: Vec<(String, bool)> = spec
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| match s.strip_prefix('-') {
            Some(rest) => (rest.trim().to_string(), true),
            None => (s.trim_start_matches('+').to_string(), false),
        })
        .collect();
    if keys.is_empty() {
        return;
    }
    items.sort_by(|a, b| {
        for (key, desc) in &keys {
            let ord = compare_by_key(a, b, key, ctx);
            if ord != Ordering::Equal {
                return if *desc { ord.reverse() } else { ord };
            }
        }
        a.id.cmp(&b.id)
    });
}

fn compare_by_key(a: &Item, b: &Item, key: &str, ctx: &Ctx) -> Ordering {
    let cfg = ctx.cfg;
    match key {
        "id" => a.id.cmp(&b.id),
        // Configured order, not alphabetical — "todo" before "doing" before "done".
        "status" => cfg
            .status_index(a.status())
            .cmp(&cfg.status_index(b.status())),
        "milestone" => milestone_rank(cfg, a.milestone()).cmp(&milestone_rank(cfg, b.milestone())),
        _ => {
            let (x, y) = (resolve(a, ctx, key), resolve(b, ctx, key));
            // Empty values sort last regardless of direction of the rest.
            match (x.is_missing(), y.is_missing()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    // Enum fields sort by their declared value order.
                    if let Some(def) = cfg.field(key)
                        && !def.values.is_empty()
                    {
                        let rank = |f: &Field| {
                            def.values
                                .iter()
                                .position(|v| v.eq_ignore_ascii_case(&f.display()))
                                .unwrap_or(usize::MAX)
                        };
                        return rank(&x).cmp(&rank(&y));
                    }
                    compare(&x.display(), &y.display())
                }
            }
        }
    }
}

/// Milestones sort in due-date order; items with no milestone go last.
pub fn milestone_rank(cfg: &Config, name: Option<&str>) -> usize {
    match name {
        None | Some("") => usize::MAX,
        Some(n) => cfg
            .milestones_ordered()
            .iter()
            .position(|m| m.name == n)
            .unwrap_or(usize::MAX - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clause(s: &str) -> Clause {
        Clause::parse(s).unwrap()
    }

    #[test]
    fn parses_every_operator() {
        assert_eq!(clause("status=todo").op, Op::Eq);
        assert_eq!(clause("status!=todo").op, Op::Ne);
        assert_eq!(clause("labels~auth").op, Op::Contains);
        assert_eq!(clause("labels!~auth").op, Op::NotContains);
        assert_eq!(clause("due>=2026-01-01").op, Op::Ge);
        assert_eq!(clause("due<2026-01-01").op, Op::Lt);
    }

    #[test]
    fn ne_is_not_read_as_eq() {
        let c = clause("priority!=p0");
        assert_eq!(c.op, Op::Ne);
        assert_eq!(c.key, "priority");
        assert_eq!(c.values, vec!["p0"]);
    }

    #[test]
    fn pipes_are_alternatives() {
        assert_eq!(clause("status=todo|doing").values, vec!["todo", "doing"]);
    }

    #[test]
    fn an_empty_value_means_the_field_is_unset() {
        let c = clause("milestone=");
        assert_eq!(c.values, vec![""]);
    }

    #[test]
    fn commas_separate_clauses() {
        let f = Filter::parse("status=todo,priority=p0").unwrap();
        assert_eq!(f.clauses.len(), 2);
    }

    #[test]
    fn a_missing_operator_is_an_error() {
        assert!(Clause::parse("status todo").is_err());
        assert!(Clause::parse("=todo").is_err());
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Filters come straight from a command line or from a model, so the
        /// parser sees arbitrary text. It may reject anything; it may not panic.
        #[test]
        fn parsing_arbitrary_expressions_never_panics(expr in "\\PC*") {
            let _ = Filter::parse(&expr);
        }

        /// Whatever it accepts, it round-trips into clauses that name a field.
        #[test]
        fn accepted_clauses_always_have_a_field(
            key in "[a-z_]{1,12}",
            op in prop_oneof![Just("="), Just("!="), Just("~"), Just("!~"), Just(">"), Just("<")],
            value in "[a-zA-Z0-9._|-]{0,20}",
        ) {
            let expr = format!("{key}{op}{value}");
            let filter = Filter::parse(&expr).unwrap();
            prop_assert_eq!(filter.clauses.len(), 1);
            prop_assert_eq!(&filter.clauses[0].key, &key);
        }
    }
}
