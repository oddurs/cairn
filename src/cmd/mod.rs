// cairn — src/cmd/mod.rs
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
pub mod board;
pub mod check;
pub mod claim;
pub mod export;
pub mod import;
pub mod init;
pub mod list;
pub mod mcp;
pub mod migrate;
pub mod milestone;
pub mod misc;
pub mod new;
pub mod next;
pub mod render_cmd;
pub mod renumber;
pub mod roadmap;
pub mod search;
pub mod set;
pub mod show;

use crate::config::{Category, Config};
use crate::item::Item;
use crate::style;

/// Colour a status using the palette declared in cairn.toml, falling back to
/// something sensible derived from its category.
/// Display text for a status — icon and label, uncoloured.
pub fn status_text(cfg: &Config, status: &str) -> String {
    match cfg.status(status) {
        Some(s) => match &s.icon {
            Some(icon) => format!("{icon} {}", s.display()),
            None => s.display().to_string(),
        },
        None => status.to_string(),
    }
}

pub fn paint_status(cfg: &Config, status: &str) -> String {
    let text = status_text(cfg, status);
    let Some(s) = cfg.status(status) else {
        return style::red(&text);
    };
    let text = text.as_str();
    match &s.color {
        Some(c) => style::named(c, text),
        None => match s.category {
            Category::Open => style::dim(text),
            Category::Active => style::yellow(text),
            Category::Done => style::green(text),
            Category::Dropped => style::dim(text),
        },
    }
}

/// Display text for a type — icon and label, uncoloured. Kept separate from
/// `paint_type` so table columns can measure the real width.
pub fn type_text(cfg: &Config, kind: Option<&str>) -> String {
    let Some(name) = kind else {
        return String::new();
    };
    match cfg.item_type(name) {
        Some(t) => {
            let label = t.label.as_deref().unwrap_or(&t.name);
            match &t.icon {
                Some(icon) => format!("{icon} {label}"),
                None => label.to_string(),
            }
        }
        None => name.to_string(),
    }
}

pub fn paint_type(cfg: &Config, kind: Option<&str>) -> String {
    let text = type_text(cfg, kind);
    let Some(name) = kind else { return text };
    match cfg.item_type(name) {
        Some(t) => match &t.color {
            Some(c) => style::named(c, &text),
            None => text,
        },
        None => style::red(&text),
    }
}

/// `[####----]  50%` — used by the roadmap view and the rendered file.
pub fn progress_bar(done: usize, total: usize, width: usize) -> String {
    if total == 0 {
        return format!("[{}]   —", "-".repeat(width));
    }
    let filled = (done * width).div_ceil(total).min(width);
    let pct = (done as f64 / total as f64 * 100.0).round() as u32;
    format!(
        "[{}{}] {pct:>3}%",
        "#".repeat(filled),
        "-".repeat(width - filled)
    )
}

pub fn count_done(cfg: &Config, items: &[&Item]) -> usize {
    items
        .iter()
        .filter(|i| cfg.category(i.status()) == Category::Done)
        .count()
}

/// One item as JSON — the interchange format for scripts and coding agents.
pub fn item_json(
    cfg: &Config,
    item: &Item,
    store: &crate::store::Store,
    body: bool,
) -> serde_json::Value {
    use serde_json::{Value as J, json};
    let mut o = serde_json::Map::new();
    o.insert("id".into(), json!(item.id));
    o.insert("ref".into(), json!(cfg.format_id(item.id)));
    o.insert("title".into(), json!(item.title()));
    o.insert("type".into(), opt_json(item.kind()));
    o.insert("status".into(), json!(item.status()));
    o.insert(
        "category".into(),
        json!(cfg.category(item.status()).as_str()),
    );
    o.insert("milestone".into(), opt_json(item.milestone()));
    o.insert("assignee".into(), opt_json(item.meta.assignee.as_deref()));
    o.insert("labels".into(), json!(item.meta.labels));
    o.insert("depends_on".into(), json!(item.meta.depends_on));
    o.insert("created".into(), opt_json(item.meta.created.as_deref()));
    o.insert("updated".into(), opt_json(item.meta.updated.as_deref()));
    o.insert("source".into(), opt_json(item.meta.source.as_deref()));
    o.insert("path".into(), json!(store.rel(&item.path)));
    let mut fields = serde_json::Map::new();
    for (k, v) in &item.meta.extra {
        if let serde_yaml_ng::Value::String(name) = k {
            fields.insert(name.clone(), yaml_to_json(v));
        }
    }
    o.insert("fields".into(), J::Object(fields));
    if body {
        o.insert("body".into(), json!(item.body));
    }
    J::Object(o)
}

fn opt_json(s: Option<&str>) -> serde_json::Value {
    match s {
        Some(v) if !v.is_empty() => serde_json::Value::String(v.to_string()),
        _ => serde_json::Value::Null,
    }
}

fn yaml_to_json(v: &serde_yaml_ng::Value) -> serde_json::Value {
    use serde_json::Value as J;
    use serde_yaml_ng::Value as Y;
    match v {
        Y::Null => J::Null,
        Y::Bool(b) => J::Bool(*b),
        Y::Number(n) => n
            .as_i64()
            .map(J::from)
            .or_else(|| {
                n.as_f64()
                    .and_then(serde_json::Number::from_f64)
                    .map(J::Number)
            })
            .unwrap_or(J::Null),
        Y::String(s) => J::String(s.clone()),
        Y::Sequence(seq) => J::Array(seq.iter().map(yaml_to_json).collect()),
        Y::Mapping(m) => J::Object(
            m.iter()
                .filter_map(|(k, v)| match k {
                    Y::String(s) => Some((s.clone(), yaml_to_json(v))),
                    _ => None,
                })
                .collect(),
        ),
        Y::Tagged(t) => yaml_to_json(&t.value),
    }
}
