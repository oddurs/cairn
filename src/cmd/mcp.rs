// cairn — Model Context Protocol server.
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
// An instruction block asks an agent to use cairn. Tools make cairn the way the
// backlog is touched at all. This is JSON-RPC 2.0 over stdio, one message per
// line, which is what the MCP stdio transport specifies.
//
// Two rules hold this together: stdout carries protocol and nothing else (every
// diagnostic goes to stderr), and a tool that fails reports the failure in its
// result rather than as a transport error, so the model can read what went
// wrong and try something else.
use crate::cmd::set::apply;
use crate::config::Config;
use crate::filter::{Ctx, Filter, sort_items};
use crate::item::{Item, parse_id, split_list};
use crate::lock::Lock;
use crate::store::{Store, today, whoami};
use crate::{Assign, hooks};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(clap::Args)]
pub struct Args {
    /// Print client configuration for this project and exit
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub config: bool,
}

pub fn run(args: Args) -> Result<i32> {
    if args.config {
        return print_config();
    }
    // Anything a hook prints would land in the middle of the protocol stream.
    hooks::silence_output();

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write(
                    &mut stdout,
                    &error_response(Value::Null, -32700, &format!("parse error: {e}")),
                )?;
                continue;
            }
        };
        // A notification has no id and takes no reply.
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(json!({}));

        let response = match method {
            "initialize" => success(id, initialize()),
            "ping" => success(id, json!({})),
            "tools/list" => success(id, json!({ "tools": tools() })),
            "tools/call" => success(id, call(&params)),
            other => error_response(id, -32601, &format!("unknown method `{other}`")),
        };
        write(&mut stdout, &response)?;
    }
    Ok(0)
}

/// The snippet a user pastes into their MCP client. Absolute paths, because
/// the client will not be running from this directory.
fn print_config() -> Result<i32> {
    let cfg = Config::discover()?;
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "cairn".to_string());
    let snippet = json!({
        "mcpServers": {
            "cairn": {
                "command": exe,
                "args": ["-C", cfg.root.display().to_string(), "mcp"],
            }
        }
    });
    println!("{}", serde_json::to_string_pretty(&snippet)?);
    eprintln!();
    eprintln!("Add this to your client's MCP configuration:");
    eprintln!("  Claude Code   .mcp.json in the project, or `claude mcp add`");
    eprintln!("  Cursor        .cursor/mcp.json");
    eprintln!("  Codex         ~/.codex/config.toml");
    Ok(0)
}

fn write(out: &mut std::io::Stdout, value: &Value) -> Result<()> {
    writeln!(out, "{value}")?;
    out.flush()?;
    Ok(())
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn initialize() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "cairn", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "This project's roadmap and issues are cairn items: Markdown files \
    in the repository under a schema defined in cairn.toml. Call get_schema first to learn the \
    project's own statuses, types and fields; they are not fixed. Use next_items to find work that \
    is ready, claim_item before starting so nobody duplicates it, update_item as you go, and \
    close_item when done. Never write TODO or PLAN files — create an item instead."
    })
}

/// Wrap a tool result. MCP expects tool failures in-band so the model can read
/// and react to them, rather than as protocol errors that abort the call.
fn ok_text(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": false })
}

fn err_text(text: String) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": true })
}

fn call(params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    match dispatch(name, &args) {
        Ok(v) => ok_text(v),
        Err(e) => err_text(format!("{e:#}")),
    }
}

fn dispatch(name: &str, a: &Value) -> Result<String> {
    match name {
        "get_schema" => get_schema(),
        "list_items" => list_items(a),
        "next_items" => next_items(a),
        "search_items" => search_items(a),
        "show_item" => show_item(a),
        "create_item" => create_item(a),
        "update_item" => update_item(a),
        "claim_item" => claim_item(a),
        "close_item" => close_item(a),
        "check" => check(),
        other => bail!("unknown tool `{other}`"),
    }
}

// --- argument helpers -------------------------------------------------------

fn s(a: &Value, key: &str) -> Option<String> {
    a.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

fn n(a: &Value, key: &str, default: usize) -> usize {
    a.get(key).and_then(Value::as_u64).unwrap_or(default as u64) as usize
}

fn b(a: &Value, key: &str) -> bool {
    a.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn require_id(a: &Value) -> Result<u32> {
    match a.get("id") {
        Some(Value::Number(v)) => Ok(v.as_u64().unwrap_or(0) as u32),
        Some(Value::String(v)) => parse_id(v),
        _ => bail!("`id` is required"),
    }
}

fn pretty(v: &Value) -> Result<String> {
    Ok(serde_json::to_string_pretty(v)?)
}

// --- tools ------------------------------------------------------------------

fn get_schema() -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_all()?;
    let ctx = Ctx::new(&cfg, &items);
    let open = items.iter().filter(|i| !ctx.is_closed(i)).count();
    let ready = items.iter().filter(|i| ctx.is_ready(i)).count();

    let mut v = crate::cmd::misc::schema_json(&cfg);
    if let Some(o) = v.as_object_mut() {
        o.insert(
            "counts".into(),
            json!({ "total": items.len(), "open": open, "ready": ready }),
        );
        o.insert(
            "filter_syntax".into(),
            json!({
                "form": "field=value, comma-separated for AND, | for alternatives",
                "operators": ["=", "!=", "~", "!~", ">", ">=", "<", "<="],
                "empty_value_means_unset": "milestone= matches items with no milestone",
                "pseudo_fields": ["category", "blocked", "ready", "blockers", "body"],
                "examples": [
                    "status=doing",
                    "priority=p0|p1,category!=done",
                    "blocked=false,milestone=v0.1",
                    "body~oauth"
                ]
            }),
        );
    }
    pretty(&v)
}

fn list_items(a: &Value) -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_all()?;
    let ctx = Ctx::new(&cfg, &items);

    let mut filter = Filter::default();
    for (key, arg) in [
        ("status", "status"),
        ("type", "type"),
        ("milestone", "milestone"),
        ("assignee", "assignee"),
        ("labels", "label"),
    ] {
        if let Some(v) = s(a, arg) {
            filter.push(key, crate::filter::Op::Eq, split_list(&v));
        }
    }
    if let Some(expr) = s(a, "filter") {
        filter = filter.and(Filter::parse(&expr)?);
    }

    let mut hits: Vec<Item> = items
        .iter()
        .filter(|i| filter.matches(i, &ctx))
        .filter(|i| b(a, "include_closed") || !ctx.is_closed(i))
        .cloned()
        .collect();
    sort_items(
        &mut hits,
        &s(a, "sort").unwrap_or_else(|| "milestone,status,id".into()),
        &ctx,
    );
    hits.truncate(n(a, "limit", 50));

    let arr: Vec<Value> = hits.iter().map(|i| enrich(&cfg, &store, &ctx, i)).collect();
    pretty(&json!({ "count": arr.len(), "items": arr }))
}

fn next_items(a: &Value) -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_all()?;
    let ctx = Ctx::new(&cfg, &items);
    let picked = crate::cmd::next::select(
        &cfg,
        &ctx,
        &items,
        &crate::cmd::next::Args {
            limit: n(a, "limit", 5),
            assignee: s(a, "assignee"),
            mine: b(a, "mine"),
            unassigned: b(a, "unassigned"),
            milestone: s(a, "milestone"),
            kind: s(a, "type"),
            filter: s(a, "filter"),
            blocked: b(a, "include_blocked"),
            json: false,
            ids: false,
        },
    )?;
    let arr: Vec<Value> = picked
        .iter()
        .map(|i| enrich(&cfg, &store, &ctx, i))
        .collect();
    pretty(&json!({ "count": arr.len(), "items": arr }))
}

fn search_items(a: &Value) -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_all()?;
    let ctx = Ctx::new(&cfg, &items);
    let Some(query) = s(a, "query") else {
        bail!("`query` is required");
    };
    let needle = query.to_lowercase();

    let hits: Vec<Value> = items
        .iter()
        .filter(|i| b(a, "include_closed") || !ctx.is_closed(i))
        .filter(|i| {
            i.title().to_lowercase().contains(&needle)
                || i.body.to_lowercase().contains(&needle)
                || i.meta
                    .labels
                    .iter()
                    .any(|l| l.to_lowercase().contains(&needle))
        })
        .take(n(a, "limit", 20))
        .map(|i| enrich(&cfg, &store, &ctx, i))
        .collect();
    pretty(&json!({ "count": hits.len(), "items": hits }))
}

fn show_item(a: &Value) -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let items = store.load_all()?;
    let ctx = Ctx::new(&cfg, &items);
    let item = store.find(require_id(a)?)?;
    let mut v = crate::cmd::item_json(&cfg, &item, &store, true);
    decorate(&mut v, &ctx, &item);
    pretty(&v)
}

fn create_item(a: &Value) -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let lock = Lock::acquire(&cfg)?;
    let existing = store.load_all()?;
    let Some(title) = s(a, "title") else {
        bail!("`title` is required");
    };

    let id = store.next_id(&existing);
    let now = today();
    let mut item = Item {
        id,
        meta: Default::default(),
        body: String::new(),
        path: store.path_for(id, &title),
        front: String::new(),
        eol: Default::default(),
    };
    item.meta.title = Some(title);
    item.meta.created = Some(now.clone());
    item.meta.updated = Some(now);

    if let Some(k) = s(a, "type").or_else(|| cfg.project.default_type.clone()) {
        apply(&mut item, &cfg, "type", Assign::Set(k))?;
    }
    apply(
        &mut item,
        &cfg,
        "status",
        Assign::Set(s(a, "status").unwrap_or_else(|| cfg.initial_status().to_string())),
    )?;
    for key in ["milestone", "assignee", "labels", "depends_on"] {
        if let Some(v) = s(a, key) {
            apply(&mut item, &cfg, key, Assign::Set(v))?;
        }
    }
    // Schema defaults first, so explicit fields win.
    for f in &cfg.fields {
        if let Some(d) = &f.default {
            apply(&mut item, &cfg, &f.name, Assign::Set(d.clone()))?;
        }
    }
    if let Some(fields) = a.get("fields").and_then(Value::as_object) {
        for (k, v) in fields {
            apply(&mut item, &cfg, k, Assign::Set(value_to_string(v)))?;
        }
    }
    for f in &cfg.fields {
        if f.required && item.get(&f.name).is_missing() {
            bail!("field `{}` is required", f.name);
        }
    }

    item.body = s(a, "body").unwrap_or_else(|| {
        item.kind()
            .and_then(|k| cfg.item_type(k))
            .and_then(|t| t.template.clone())
            .unwrap_or_default()
    });
    if item.path.exists() {
        bail!("{} already exists", item.path.display());
    }
    item.save()?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterCreate, &item);

    pretty(&json!({
        "created": cfg.format_id(item.id),
        "id": item.id,
        "path": store.rel(&item.path),
    }))
}

fn update_item(a: &Value) -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let lock = Lock::acquire(&cfg)?;
    let mut item = store.find(require_id(a)?)?;

    let Some(fields) = a.get("fields").and_then(Value::as_object) else {
        bail!("`fields` is required: an object of field names to values");
    };
    for (k, v) in fields {
        apply(&mut item, &cfg, k, Assign::Set(value_to_string(v)))?;
    }
    if let Some(body) = a.get("body").and_then(Value::as_str) {
        item.body = body.to_string();
    }
    item.touch(&today());
    item.save()?;
    store.sync_path(&mut item)?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &item);

    let items = store.load_all()?;
    let ctx = Ctx::new(&cfg, &items);
    let mut v = crate::cmd::item_json(&cfg, &item, &store, false);
    decorate(&mut v, &ctx, &item);
    pretty(&v)
}

fn claim_item(a: &Value) -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let lock = Lock::acquire(&cfg)?;
    let items = store.load_all()?;
    let ctx = Ctx::new(&cfg, &items);

    let id = match a.get("id") {
        Some(Value::Null) | None => {
            let picked = crate::cmd::next::select(
                &cfg,
                &ctx,
                &items,
                &crate::cmd::next::Args {
                    limit: 1,
                    assignee: None,
                    mine: false,
                    unassigned: true,
                    milestone: s(a, "milestone"),
                    kind: s(a, "type"),
                    filter: s(a, "filter"),
                    blocked: false,
                    json: false,
                    ids: false,
                },
            )?;
            match picked.first() {
                Some(i) => i.id,
                None => bail!("nothing unclaimed is ready to start"),
            }
        }
        _ => require_id(a)?,
    };

    let mut item = store.find(id)?;
    let who = s(a, "as").unwrap_or_else(whoami);
    let force = b(a, "force");

    if let Some(holder) = item.meta.assignee.as_deref()
        && !holder.is_empty()
        && !holder.eq_ignore_ascii_case(&who)
        && !force
    {
        bail!(
            "{} is already claimed by {holder}; pass force to take it anyway",
            cfg.format_id(id)
        );
    }
    let blockers = ctx.blockers(&item);
    if !blockers.is_empty() && !force {
        let list: Vec<String> = blockers.iter().map(|x| cfg.format_id(*x)).collect();
        bail!("{} is blocked by {}", cfg.format_id(id), list.join(", "));
    }

    let status = match s(a, "status") {
        Some(v) => v,
        None => match cfg
            .statuses
            .iter()
            .find(|st| st.category == crate::config::Category::Active)
        {
            Some(st) => st.name.clone(),
            None => bail!("no `active` status is defined in cairn.toml"),
        },
    };
    apply(&mut item, &cfg, "assignee", Assign::Set(who.clone()))?;
    apply(&mut item, &cfg, "status", Assign::Set(status))?;
    item.touch(&today());
    item.save()?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &item);

    pretty(&json!({
        "claimed": cfg.format_id(item.id),
        "id": item.id,
        "title": item.title(),
        "assignee": who,
        "status": item.status(),
        "path": store.rel(&item.path),
        "body": item.body,
    }))
}

fn close_item(a: &Value) -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let lock = Lock::acquire(&cfg)?;
    let mut item = store.find(require_id(a)?)?;
    let status = match s(a, "status") {
        Some(v) => v,
        None => match cfg.done_status() {
            Some(st) => st.name.clone(),
            None => bail!("no `done` status is defined in cairn.toml"),
        },
    };
    apply(&mut item, &cfg, "status", Assign::Set(status))?;
    item.touch(&today());
    item.save()?;
    drop(lock);
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &item);
    pretty(&json!({ "closed": cfg.format_id(item.id), "status": item.status() }))
}

fn check() -> Result<String> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let report = crate::cmd::check::collect(&cfg, &store)?;
    pretty(&json!({
        "ok": report.errors.is_empty(),
        "errors": report.errors,
        "warnings": report.warnings,
    }))
}

// --- shared shaping ---------------------------------------------------------

fn enrich(cfg: &Config, store: &Store, ctx: &Ctx, item: &Item) -> Value {
    let mut v = crate::cmd::item_json(cfg, item, store, false);
    decorate(&mut v, ctx, item);
    v
}

/// Dependency state is the thing an agent most needs and cannot compute from
/// one item, so every item that crosses this boundary carries it.
fn decorate(v: &mut Value, ctx: &Ctx, item: &Item) {
    if let Some(o) = v.as_object_mut() {
        o.insert("blockers".into(), json!(ctx.blockers(item)));
        o.insert("blocked".into(), json!(ctx.is_blocked(item)));
        o.insert("ready".into(), json!(ctx.is_ready(item)));
    }
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Array(a) => a.iter().map(value_to_string).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

// --- tool definitions -------------------------------------------------------

fn obj(props: Value, required: Vec<&str>) -> Value {
    json!({ "type": "object", "properties": props, "required": required })
}

fn str_prop(desc: &str) -> Value {
    json!({ "type": "string", "description": desc })
}

fn int_prop(desc: &str) -> Value {
    json!({ "type": "integer", "description": desc })
}

fn bool_prop(desc: &str) -> Value {
    json!({ "type": "boolean", "description": desc })
}

const FILTER_HELP: &str = "Filter expression: field=value, comma-separated for AND, | for \
alternatives. Operators = != ~ !~ > >= < <=. An empty value means unset. Pseudo-fields: \
category, blocked, ready, body. Example: priority=p0|p1,blocked=false";

fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "get_schema",
            "description": "The project's schema — its item types, statuses, custom fields, \
        milestones and saved views — plus current counts and the filter syntax. Call this first: the \
        statuses and fields are defined per project and are not fixed.",
            "inputSchema": obj(json!({}), vec![]),
        }),
        json!({
            "name": "next_items",
            "description": "Work that is ready to start: not finished, and with no unfinished \
        dependencies. Ranked with work already in progress first, then by priority. This is the right \
        way to answer \"what should I do next?\".",
            "inputSchema": obj(json!({
                "limit": int_prop("How many to return (default 5)"),
                "milestone": str_prop("Restrict to one milestone"),
                "type": str_prop("Restrict to one item type"),
                "assignee": str_prop("Only work assigned to this person"),
                "mine": bool_prop("Only work assigned to you or to nobody"),
                "unassigned": bool_prop("Only work with no assignee"),
                "filter": str_prop(FILTER_HELP),
                "include_blocked": bool_prop("Include blocked work, with its blockers"),
            }), vec![]),
        }),
        json!({
            "name": "list_items",
            "description": "Query the backlog. Finished items are excluded unless \
        include_closed is set.",
            "inputSchema": obj(json!({
                "status": str_prop("One or more statuses, comma-separated"),
                "type": str_prop("One or more types, comma-separated"),
                "milestone": str_prop("One or more milestones, comma-separated"),
                "label": str_prop("One or more labels, comma-separated"),
                "assignee": str_prop("Assignee"),
                "filter": str_prop(FILTER_HELP),
                "sort": str_prop("Sort keys, '-' prefix for descending. Default milestone,status,id"),
                "limit": int_prop("Maximum items to return (default 50)"),
                "include_closed": bool_prop("Include done and dropped items"),
            }), vec![]),
        }),
        json!({
            "name": "search_items",
            "description": "Full-text search over item titles, bodies and labels.",
            "inputSchema": obj(json!({
                "query": str_prop("Text to look for, case-insensitive"),
                "limit": int_prop("Maximum items to return (default 20)"),
                "include_closed": bool_prop("Include done and dropped items"),
            }), vec!["query"]),
        }),
        json!({
            "name": "show_item",
            "description": "One item in full, including its Markdown body, its dependencies \
        and whether it is blocked.",
            "inputSchema": obj(json!({ "id": int_prop("Item id") }), vec!["id"]),
        }),
        json!({
            "name": "claim_item",
            "description": "Take an item before working on it: assigns it to you and moves it \
        to an active status, so no one else starts the same work. Omit id to claim the next ready \
        unclaimed item. Refuses an item someone else holds, or one that is blocked, unless force is \
        set. Returns the item's body so you can start immediately.",
            "inputSchema": obj(json!({
                "id": int_prop("Item id; omit to take the next ready one"),
                "as": str_prop("Claim as this name (default: CAIRN_USER, else git user.name)"),
                "status": str_prop("Status to move to (default: the first active status)"),
                "milestone": str_prop("When picking automatically, restrict to this milestone"),
                "type": str_prop("When picking automatically, restrict to this type"),
                "filter": str_prop(FILTER_HELP),
                "force": bool_prop("Take it even if held by someone else or blocked"),
            }), vec![]),
        }),
        json!({
            "name": "create_item",
            "description": "Add an item to the backlog. Use this instead of writing a TODO, \
        PLAN or NOTES file. Field values are validated against the schema, so call get_schema first if \
        you are unsure what a status or field accepts.",
            "inputSchema": obj(json!({
                "title": str_prop("One-line title"),
                "type": str_prop("Item type, from the schema"),
                "status": str_prop("Initial status (default: the project's default)"),
                "milestone": str_prop("Milestone name, from the schema"),
                "labels": str_prop("Comma-separated labels"),
                "assignee": str_prop("Who owns it"),
                "depends_on": str_prop("Comma-separated ids this item depends on"),
                "body": str_prop("Markdown body: problem, proposal, acceptance criteria"),
                "fields": json!({
                    "type": "object",
                    "description": "Custom fields declared in the schema, e.g. {\"priority\":\"p0\"}",
                }),
            }), vec!["title"]),
        }),
        json!({
            "name": "update_item",
            "description": "Change fields on an item. Every value is validated against the \
        schema before anything is written.",
            "inputSchema": obj(json!({
                "id": int_prop("Item id"),
                "fields": json!({
                    "type": "object",
                    "description": "Field names to values, e.g. {\"status\":\"doing\",\"priority\":\"p0\"}. \
        An empty string clears a field.",
                }),
                "body": str_prop("Replace the Markdown body"),
            }), vec!["id", "fields"]),
        }),
        json!({
            "name": "close_item",
            "description": "Mark an item finished.",
            "inputSchema": obj(json!({
                "id": int_prop("Item id"),
                "status": str_prop("Status to move to (default: the first done status)"),
            }), vec!["id"]),
        }),
        json!({
            "name": "check",
            "description": "Validate every item against the schema: unknown statuses and \
        fields, missing required values, dangling or circular dependencies, duplicate ids. Run this \
        before reporting work as finished.",
            "inputSchema": obj(json!({}), vec![]),
        }),
    ]
}
