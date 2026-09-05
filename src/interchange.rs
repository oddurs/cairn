// cairn — the interchange format.
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
// One documented format sits between cairn and every tracker it will ever talk
// to. Adapters produce it or consume it; the core knows nothing about GitHub or
// anything else. That keeps a proprietary forge out of the core, and it means a
// tracker cairn has never heard of is one `jq` script away from working.
//
// The load-bearing detail is `category`. Statuses are named per project, so an
// importer cannot match them by name across a boundary — but every status
// belongs to one of four categories, and those are fixed. An item arriving from
// anywhere can always be placed by category even when its status name means
// nothing here.
use crate::cmd::item_json;
use crate::config::Config;
use crate::filter::Ctx;
use crate::item::Item;
use crate::store::Store;
use serde::Deserialize;
use serde_json::{Value, json};

/// Interchange format version. Bumped only for changes a consumer must know
/// about; new optional keys do not bump it.
pub const FORMAT_VERSION: &str = "1";

/// Build the interchange document for a set of items.
pub fn document(cfg: &Config, store: &Store, items: &[Item], today: &str) -> Value {
    let ctx = Ctx::new(cfg, items);
    let entries: Vec<Value> = items
        .iter()
        .map(|i| {
            let mut v = item_json(cfg, i, store, true);
            if let Some(o) = v.as_object_mut() {
                // The path is local detail; provenance travels instead.
                o.remove("path");
                o.insert("blocked".into(), json!(ctx.is_blocked(i)));
            }
            v
        })
        .collect();

    json!({
        "cairn": FORMAT_VERSION,
        "exported": today,
        "project": {
            "name": cfg.project.name,
            "description": cfg.project.description,
            "url": cfg.project.url,
        },
        // The schema travels with the items so a consumer can interpret status
        // and field names it has never seen.
        "schema": crate::cmd::misc::schema_json(cfg),
        "items": entries,
    })
}

/// One item as it arrives. Everything is optional: a document from an adapter
/// somebody wrote in an afternoon should still import as far as it can.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Incoming {
    #[serde(default)]
    pub id: Option<u32>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    /// open / active / done / dropped. The fallback when `status` is a name
    /// this project does not use.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub milestone: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignee: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<u32>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub fields: std::collections::BTreeMap<String, Value>,
}

/// Accepts a full interchange document, or a bare array of items, or a single
/// item. Being liberal here costs nothing and saves every adapter author a
/// wrapper.
pub fn items_from(doc: &Value) -> anyhow::Result<Vec<Incoming>> {
    let raw = match doc {
        Value::Object(o) if o.contains_key("items") => o.get("items").cloned().unwrap_or(json!([])),
        Value::Object(_) => json!([doc]),
        Value::Array(_) => doc.clone(),
        _ => anyhow::bail!("expected a JSON object or array of items"),
    };
    let items: Vec<Incoming> = serde_json::from_value(raw)?;
    Ok(items)
}
