// cairn — src/cmd/mod.rs
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
pub mod board;
pub mod check;
pub mod claim;
pub mod export;
pub mod git;
pub mod import;
pub mod init;
pub mod list;
pub mod log;
pub mod mcp;
pub mod migrate;
pub mod misc;
pub mod new;
pub mod next;
pub mod note;
pub mod propose;
pub mod render_cmd;
pub mod renumber;
pub mod roadmap;
pub mod search;
pub mod set;
pub mod show;
pub mod tick;

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
        None => match s.category() {
            Category::Active => style::yellow(text),
            Category::Done => style::green(text),
            Category::Open | Category::Dropped => style::dim(text),
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
/// Grouping items whose work is all finished while they are still open.
///
/// Reported, never acted on. Closing one automatically would be cairn deciding
/// that finished work means a shipped milestone, and those are different claims:
/// a project's `later` or `Someday` milestone can have every item under it done
/// and be meant to stay open for good. The item is the place that judgement
/// lives, so cairn says what it can see and leaves the decision.
///
/// A grouping item with nothing under it is not reported. Nothing filed is
/// vacuously complete, and every empty milestone in a young project would
/// otherwise be told it was finished the day it was written.
pub fn finished_but_open<'a>(cfg: &Config, items: &'a [Item]) -> Vec<(&'a Item, usize)> {
    let mut out = Vec::new();
    for container in items.iter().filter(|i| cfg.is_container(i.kind())) {
        if cfg.category(container.status()).is_closed() {
            continue;
        }
        let Some(key) = container.key() else { continue };
        let under: Vec<&Item> = items
            .iter()
            .filter(|i| !cfg.is_container(i.kind()))
            .filter(|i| {
                cfg.schedule_of(i)
                    .is_some_and(|v| v.eq_ignore_ascii_case(key))
            })
            .collect();
        let (done, total) = progress(cfg, &under);
        if total > 0 && done == total {
            out.push((container, total));
        }
    }
    out
}

/// What to say about one of them: the fact, and the command, and the reason the
/// command is not run for you.
pub fn finished_but_open_line(cfg: &Config, container: &Item) -> String {
    format!(
        "nothing unfinished — {} when it has shipped",
        style::dim(&format!("`cairn close {}`", cfg.format_id(container.id)))
    )
}

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

/// Finished and countable, for a progress bar.
///
/// Dropped items are excluded from both. A milestone holding three abandoned
/// ideas and one finished item is complete, not a quarter done, and reporting
/// it as a quarter done makes the number worthless — the reader has to open the
/// milestone to find out whether the remainder is work or wreckage.
pub fn progress(cfg: &Config, items: &[&Item]) -> (usize, usize) {
    let countable: Vec<&&Item> = items
        .iter()
        .filter(|i| cfg.category(i.status()) != Category::Dropped)
        .collect();
    let done = countable
        .iter()
        .filter(|i| cfg.category(i.status()) == Category::Done)
        .count();
    (done, countable.len())
}

/// One item as JSON — the interchange format for scripts and coding agents.
/// The one-line shape of a set of items: what can be started, what is under
/// way, what is waiting. Shown under `next` and `board` so the two agree.
///
/// Zero terms are omitted rather than printed as zeroes — "0 blocked" is not
/// information, and a summary that is mostly zeroes stops being read.
pub fn summary(ctx: &crate::filter::Ctx, items: &[&Item]) -> String {
    // Over work only. A container counted as ready would make the line
    // disagree with the table above it, which is worse than no line.
    let items: Vec<&&Item> = items
        .iter()
        .filter(|i| !ctx.cfg.is_container(i.kind()))
        .collect();
    let ready = items.iter().filter(|i| ctx.is_ready(i)).count();
    let active = items
        .iter()
        .filter(|i| ctx.cfg.category(i.status()) == Category::Active)
        .count();
    let blocked = items.iter().filter(|i| ctx.is_blocked(i)).count();

    let mut parts = Vec::new();
    if ready > 0 {
        parts.push(format!("{ready} ready"));
    }
    if active > 0 {
        parts.push(format!("{active} in progress"));
    }
    if blocked > 0 {
        parts.push(format!("{blocked} blocked"));
    }
    if parts.is_empty() {
        "nothing to start".to_string()
    } else {
        parts.join(" · ")
    }
}

/// Fields the schema marks as worth showing in a table. The same choice `list`
/// makes, so a project that tracks something other than priority gets the same
/// treatment everywhere without configuring it twice.
pub fn table_fields(cfg: &Config) -> Vec<String> {
    cfg.fields
        .iter()
        .filter(|f| f.column)
        .map(|f| f.name.clone())
        .collect()
}

/// Say that a title changed and the filename did not follow it.
///
/// Every caller of `sync_path` saves the item first, so this is always news
/// about something that has already happened. Reporting it as a failure — which
/// `cairn set` did — told a reader the change had not been made while the file
/// on disk said it had, and running the command again said the same thing
/// forever.
///
/// One wording in one place, because five commands say it.
pub fn note_if_blocked(store: &crate::store::Store, item: &Item, outcome: &crate::store::Renamed) {
    if let crate::store::Renamed::Blocked(taken) = outcome {
        eprintln!(
            "{} {} keeps the filename {}: {} is taken by another file.\n  \
             The change was written. A filename is cosmetic — `cairn check` \
             reports the drift, and nothing was overwritten.",
            style::yellow("note:"),
            cfg_id(store, item),
            style::dim(&store.rel(&item.path)),
            style::dim(&store.rel(taken)),
        );
    }
}

/// An item's identifier as the project renders it.
fn cfg_id(store: &crate::store::Store, item: &Item) -> String {
    style::bold(&store.cfg.format_id(item.id))
}

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
    o.insert("key".into(), opt_json(item.key()));
    o.insert("title".into(), json!(item.title()));
    o.insert("type".into(), opt_json(item.kind()));
    o.insert("status".into(), json!(item.status()));
    o.insert(
        "category".into(),
        json!(cfg.category(item.status()).as_str()),
    );
    o.insert("milestone".into(), opt_json(item.milestone()));
    o.insert("assignee".into(), opt_json(item.meta.assignee.as_deref()));
    // Who is answerable, and what made it. Writable and shown as columns since
    // they arrived, and absent from here — so an export dropped them and a
    // round trip through the interchange document lost them silently.
    o.insert("claimed".into(), opt_json(item.meta.claimed.as_deref()));
    o.insert("owner".into(), opt_json(item.meta.owner.as_deref()));
    o.insert(
        "created_by".into(),
        opt_json(item.meta.created_by.as_deref()),
    );
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

/// One line naming items that look like the one being filed.
///
/// Shared by `cairn new` and `create_item`, because the agent path is the one
/// this exists for and a warning on standard error is not something a model
/// reads — over the protocol it has to be in the result.
pub fn similar_line(cfg: &crate::config::Config, similar: &[&crate::item::Item]) -> String {
    let named: Vec<String> = similar
        .iter()
        .map(|i| format!("{} \"{}\"", cfg.format_id(i.id), i.title()))
        .collect();
    format!(
        "{} look{} similar — check before filing another",
        named.join(" and "),
        if similar.len() == 1 { "s" } else { "" }
    )
}

/// A date, said the way `updated` records one.
///
/// Day granularity, because that is all `updated` carries. Anything finer would
/// be a lie, and pretending otherwise would push cairn towards timestamps in
/// frontmatter that a person has to read.
pub fn a_date(raw: &str) -> anyhow::Result<String> {
    let text = raw.trim();
    if chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok() {
        return Ok(text.to_string());
    }
    anyhow::bail!("`{raw}` is not a date; use YYYY-MM-DD")
}
