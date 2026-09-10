// cairn — src/cmd/misc.rs
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
// cairn config / agent / completions / man.
use crate::BUG_ADDRESS;
use crate::config::{CONFIG_FILE, Config, FieldKind};
use crate::style;
use anyhow::Result;
use clap::{ArgAction, CommandFactory};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct ConfigArgs {
    /// Print the path to cairn.toml and exit
    #[arg(long, action = ArgAction::SetTrue)]
    pub path: bool,

    /// Print the resolved schema as JSON
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

#[derive(clap::Args)]
pub struct AgentArgs {
    /// Insert or update the block in this file instead of printing it
    #[arg(short, long, value_name = "FILE")]
    pub write: Option<PathBuf>,
}

#[derive(clap::Args)]
pub struct CompletionsArgs {
    /// Shell to generate for
    #[arg(value_name = "SHELL")]
    pub shell: clap_complete::Shell,
}

#[derive(clap::Args)]
pub struct ManArgs {
    /// Write one page per subcommand into DIR instead of stdout
    #[arg(short, long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
}

pub fn config(args: ConfigArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    if args.path {
        println!("{}", cfg.root.join(CONFIG_FILE).display());
        return Ok(0);
    }
    // Milestones are items, so describing the schema now needs the backlog.
    // Read leniently: a project with one unreadable item should still be able
    // to say what its schema is.
    let items = crate::store::Store::new(&cfg)
        .load_lenient()
        .map(|(items, _)| items)
        .unwrap_or_default();
    let milestones = crate::refs::Milestones::new(&cfg, &items);
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&schema_json(&cfg, &milestones))?
        );
        return Ok(0);
    }

    println!(
        "{}  {}",
        style::bold(&cfg.project.name),
        style::dim(&cfg.root.join(CONFIG_FILE).display().to_string())
    );
    if let Some(d) = &cfg.project.description {
        println!("{}", style::dim(d));
    }
    println!();
    println!("{:<12} {}", style::dim("items"), cfg.project.dir);
    println!("{:<12} {}", style::dim("render"), cfg.render.target);
    println!(
        "{:<12} {}",
        style::dim("git"),
        if crate::cmd::git::is_configured(&cfg) {
            "integrated".to_string()
        } else if crate::cmd::git::in_repository(&cfg.root) {
            style::dim("not integrated — `cairn init --git`")
        } else {
            style::dim("not a repository")
        }
    );
    println!();

    section(
        "types",
        cfg.types.iter().map(|t| {
            let mut s = t.name.clone();
            if let Some(d) = &t.description {
                s.push_str(&format!("  {}", style::dim(d)));
            }
            s
        }),
    );
    section(
        "statuses",
        cfg.statuses.iter().map(|s| {
            // An assumed category is shown as an assumption. The resolved
            // schema must not present a guess as a decision — `open` for a
            // status somebody named `shipped` is the one default in this file
            // that can be silently, consequentially wrong.
            let category = match s.declared_category {
                Some(c) => format!("category = {}", c.as_str()),
                None => format!("category = {} (assumed)", s.category().as_str()),
            };
            format!("{:<12} {}", s.name, style::dim(&category))
        }),
    );
    section(
        "fields",
        cfg.fields.iter().map(|f| {
            let mut s = format!("{:<12} {}", f.name, describe_field(f));
            if f.required {
                s.push_str(&style::yellow("  required"));
            }
            s
        }),
    );
    section(
        "milestones",
        milestones.iter().map(|m| {
            format!(
                "{:<12} {}",
                m.key().unwrap_or_default(),
                style::dim(&{
                    let mut bits: Vec<String> = vec![m.title().to_string()];
                    if let Some(d) = crate::refs::due(m) {
                        bits.push(format!("due {d}"));
                    }
                    bits.push(format!("[{}]", m.status()));
                    bits.join(" — ")
                })
            )
        }),
    );
    section(
        "views",
        cfg.views.iter().map(|v| {
            format!(
                "{:<12} {}",
                v.name,
                style::dim(v.filter.as_deref().unwrap_or(""))
            )
        }),
    );
    Ok(0)
}

fn section(title: &str, rows: impl Iterator<Item = String>) {
    let rows: Vec<String> = rows.collect();
    if rows.is_empty() {
        return;
    }
    println!("{}", style::bold(title));
    for r in rows {
        println!("  {r}");
    }
    println!();
}

/// A ref in one phrase, for a person reading `cairn config` and for the
/// instruction block a model reads. Both get the same words, because two
/// vocabularies for one idea is what this mechanism exists to avoid.
fn describe_ref(f: &crate::config::FieldDef, verb: &str) -> String {
    let what = match f.target.as_deref() {
        Some("*") | None => "any item".to_string(),
        Some(t) => format!("a `{t}` item"),
    };
    let how = match f.by {
        crate::config::Addressing::Key => "by key",
        crate::config::Addressing::Id => "by id",
    };
    match f.cardinality {
        crate::config::Cardinality::One => format!("{verb} {what}, {how}"),
        crate::config::Cardinality::Many => format!("{verb} {what}s, {how}, several allowed"),
    }
}

fn describe_field(f: &crate::config::FieldDef) -> String {
    let kind = match f.kind {
        FieldKind::Enum => format!("one of: {}", f.values.join(", ")),
        FieldKind::Text => "text".into(),
        FieldKind::List => {
            if f.values.is_empty() {
                "list".into()
            } else {
                format!("list of: {}", f.values.join(", "))
            }
        }
        FieldKind::Ref => describe_ref(f, "names"),
        FieldKind::Date => "date (YYYY-MM-DD)".into(),
        FieldKind::Number => "number".into(),
        FieldKind::Bool => "true / false".into(),
    };
    style::dim(&kind)
}

pub fn schema_json(cfg: &Config, milestones: &crate::refs::Milestones) -> serde_json::Value {
    use serde_json::json;
    json!({
        "project": {
            "name": cfg.project.name,
            "description": cfg.project.description,
            "dir": cfg.project.dir,
            "id_width": cfg.project.id_width,
            "default_type": cfg.project.default_type,
            "default_status": cfg.initial_status(),
            "root": cfg.root.display().to_string(),
            "git_integrated": crate::cmd::git::is_configured(cfg),
        },
        "types": cfg.types.iter().map(|t| json!({
            "name": t.name, "label": t.label, "description": t.description
        })).collect::<Vec<_>>(),
        "statuses": cfg.statuses.iter().map(|s| json!({
            "name": s.name, "label": s.label, "category": s.category().as_str(),
            "category_declared": s.declared_category.is_some(), "board": s.board
        })).collect::<Vec<_>>(),
        // Declared fields and the built-in refs in one list, so a model meets
        // one vocabulary rather than a general mechanism plus a special case.
        "fields": cfg.fields.iter().cloned()
            .chain(cfg.builtin_ref_fields())
            .map(|f| {
                let mut v = json!({
                    "name": f.name,
                    "kind": format!("{:?}", f.kind).to_lowercase(),
                    "values": f.values,
                    "required": f.required,
                    "default": f.default,
                    "description": f.description,
                });
                if f.kind == FieldKind::Ref {
                    v["target"] = json!(f.target.as_deref().unwrap_or("*"));
                    v["cardinality"] = json!(match f.cardinality {
                        crate::config::Cardinality::One => "one",
                        crate::config::Cardinality::Many => "many",
                    });
                    v["by"] = json!(match f.by {
                        crate::config::Addressing::Key => "key",
                        crate::config::Addressing::Id => "id",
                    });
                    v["acyclic"] = json!(f.acyclic);
                    v["rollup"] = json!(f.rollup);
                    v["inverse"] = json!(f.inverse);
                }
                v
            }).collect::<Vec<_>>(),
        // A milestone is an item, so this reports what it is rather than a
        // separate shape: the key it answers to, and where to read the rest.
        "milestones": milestones.iter().map(|m| json!({
            "id": m.id,
            "key": m.key(),
            "title": m.title(),
            "due": crate::refs::due(m),
            "status": m.status(),
        })).collect::<Vec<_>>(),
        "views": cfg.views.iter().map(|v| json!({
            "name": v.name, "description": v.description, "filter": v.filter, "sort": v.sort
        })).collect::<Vec<_>>(),
        "render": { "target": cfg.render.target, "group_by": cfg.render.group_by },
    })
}

const BEGIN: &str = "<!-- cairn:begin -->";
const END: &str = "<!-- cairn:end -->";

pub fn agent(args: AgentArgs) -> Result<i32> {
    let cfg = Config::discover()?;
    let items = crate::store::Store::new(&cfg)
        .load_lenient()
        .map(|(items, _)| items)
        .unwrap_or_default();
    let block = agent_block(&cfg, &crate::refs::Milestones::new(&cfg, &items));

    let Some(path) = args.write else {
        print!("{block}");
        return Ok(0);
    };

    let full = if path.is_absolute() {
        path.clone()
    } else {
        cfg.root.join(&path)
    };
    let existing = std::fs::read_to_string(&full).unwrap_or_default();
    // Replace an existing block in place so the file can be edited around it.
    let updated = match (existing.find(BEGIN), existing.find(END)) {
        (Some(a), Some(b)) if b > a => {
            format!(
                "{}{}{}",
                &existing[..a],
                block.trim_end(),
                &existing[b + END.len()..]
            )
        }
        _ => {
            let mut s = existing;
            if !s.is_empty() && !s.ends_with("\n\n") {
                s.push_str(if s.ends_with('\n') { "\n" } else { "\n\n" });
            }
            s.push_str(&block);
            s
        }
    };
    crate::store::write_atomic(&full, updated.as_bytes())?;
    println!("{} {}", style::green("wrote"), full.display());
    Ok(0)
}

/// The instructions block. Generated from the live schema so it can never
/// describe a workflow the project does not actually have.
fn agent_block(cfg: &Config, milestones: &crate::refs::Milestones) -> String {
    let mut s = String::new();
    s.push_str(BEGIN);
    s.push_str("\n## Roadmap and issues\n\n");
    s.push_str(&format!(
        "This project tracks its roadmap and issues with `cairn`. Every item is a Markdown file \
         under `{}`, described by the schema in `{CONFIG_FILE}`.\n\n",
        cfg.project.dir
    ));
    s.push_str(
        "**Do not create ad-hoc TODO, PLAN or NOTES files.** Create a cairn item instead, so the \
         work appears on the board and in the generated roadmap.\n\n",
    );

    s.push_str("### The loop\n\n");
    s.push_str(
        "1. `cairn next` — what is ready to start. It excludes anything blocked by unfinished \
dependencies and puts work already in progress first.\n",
    );
    s.push_str(
        "2. `cairn claim <ID>` — take it before you start, so no one duplicates the work. \
`cairn claim --next` picks and claims the top-ranked unclaimed item in one step, and prints its \
body so you can begin immediately.\n",
    );
    s.push_str(
        "3. Do the work. Record what you learn: `cairn set <ID> <field>=<value>` for fields, \
`cairn note <ID> \"<TEXT>\"` for anything that needs a sentence — why you chose something, what \
you tried, what to watch for.\n",
    );
    s.push_str("4. `cairn close <ID>` when it is done, or `cairn release <ID>` to hand it back.\n");
    s.push_str("5. `cairn check` before you report finished. It must pass.\n\n");

    s.push_str("### Commands\n\n```sh\n");
    s.push_str("cairn next --json                 # ready work, ranked\n");
    s.push_str("cairn claim --next                # take the next ready item\n");
    s.push_str("cairn search <TEXT> --json        # titles, bodies and labels\n");
    s.push_str("cairn list --json                 # all open items\n");
    s.push_str("cairn list --filter 'blocked=false,priority=p0'\n");
    s.push_str("cairn show <ID> --json            # one item, including its body\n");
    s.push_str("cairn new \"<TITLE>\" --type <TYPE> --milestone <MILESTONE>\n");
    s.push_str("cairn set <ID> status=<STATUS>    # also labels+=x, or any field below\n");
    s.push_str("cairn note <ID> \"<TEXT>\"          # append reasoning; never replaces\n");
    s.push_str("cairn close <ID>\n");
    s.push_str("cairn check                       # validate; run before finishing\n");
    s.push_str(&format!(
        "cairn render                      # regenerate {}\n```\n\n",
        cfg.render.target
    ));

    s.push_str("### Schema\n\n");
    if !cfg.types.is_empty() {
        s.push_str(&format!(
            "- **Types**: {}\n",
            cfg.types
                .iter()
                .map(|t| format!("`{}`", t.name))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    s.push_str(&format!(
        "- **Statuses**: {}\n",
        cfg.statuses
            .iter()
            .map(|st| format!("`{}` ({})", st.name, st.category().as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    for f in &cfg.fields {
        let restriction = match f.agent {
            crate::config::Agent::Write => String::new(),
            crate::config::Agent::ReadOnly => " — **you may read this and not set it**".into(),
            crate::config::Agent::Propose => {
                " — **you may propose this in a note, not set it**".into()
            }
        };
        s.push_str(&format!(
            "- **`{}`**: {}{}{}\n",
            f.name,
            match f.kind {
                FieldKind::Enum => format!("one of {}", f.values.join(", ")),
                FieldKind::List => "list of values".to_string(),
                FieldKind::Date => "date, YYYY-MM-DD".to_string(),
                FieldKind::Number => "number".to_string(),
                FieldKind::Bool => "true or false".to_string(),
                FieldKind::Text => "free text".to_string(),
                FieldKind::Ref => describe_ref(f, "names"),
            },
            if f.required { " (required)" } else { "" },
            f.description
                .as_ref()
                .map(|d| format!(" — {d}{restriction}"))
                .unwrap_or_else(|| restriction.clone()),
        ));
    }
    if !milestones.is_empty() {
        s.push_str(&format!(
            "- **Milestones**: {}\n",
            milestones
                .iter()
                .map(|m| match crate::refs::due(m) {
                    Some(d) => format!("`{}` (due {d})", m.key().unwrap_or_default()),
                    None => format!("`{}`", m.key().unwrap_or_default()),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !cfg.views.is_empty() {
        s.push_str(&format!(
            "- **Saved views** (`cairn list --view NAME`): {}\n",
            cfg.views
                .iter()
                .map(|v| format!("`{}`", v.name))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    s.push_str("\n### Rules\n\n");
    s.push_str(
        "1. Before starting work, find or create the item and set it to an active status.\n",
    );
    s.push_str("2. Use the fields above rather than inventing new ones; add new fields to `cairn.toml` first.\n");
    s.push_str(
        "3. Never hand-edit the generated roadmap file — change items and run `cairn render`.\n",
    );
    s.push_str("4. `cairn check` must pass before the work is considered done.\n\n");
    s.push_str(END);
    s.push('\n');
    s
}

pub fn completions<C: CommandFactory>(args: CompletionsArgs) -> Result<i32> {
    let mut cmd = C::command();
    clap_complete::generate(args.shell, &mut cmd, "cairn", &mut std::io::stdout());
    Ok(0)
}

pub fn man<C: CommandFactory>(args: ManArgs) -> Result<i32> {
    let cmd = C::command();
    match args.dir {
        Some(dir) => {
            std::fs::create_dir_all(&dir)?;
            clap_mangen::generate_to(cmd, &dir)?;
            println!("{} {}", style::green("wrote man pages to"), dir.display());
        }
        None => {
            let mut out = std::io::stdout();
            clap_mangen::Man::new(cmd).render(&mut out)?;
        }
    }
    Ok(0)
}

/// The environment a maintainer asks for, printed so it can be pasted into an
/// issue without a conversation first.
///
/// Deliberately says nothing a reporter would not want to publish: no paths
/// outside the project, no environment beyond the platform, no item titles.
/// Somebody pastes this into a public tracker, and a diagnostic that leaks is
/// a diagnostic nobody runs twice.
pub fn bug_report() -> Result<i32> {
    println!("cairn {}", env!("CARGO_PKG_VERSION"));
    println!(
        "platform: {} {} ({})",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::FAMILY
    );

    match std::env::current_dir().ok().and_then(|d| Config::find(&d)) {
        None => println!("project: none found from the current directory"),
        Some(path) => match Config::load(&path) {
            // A project that will not load is the interesting case, so say so
            // rather than failing: this command runs when something is wrong.
            Err(e) => println!("project: {CONFIG_FILE} present, but will not load: {e:#}"),
            Ok(cfg) => {
                println!("format: {}", cfg.format());
                println!(
                    "schema: {} type(s), {} status(es), {} extra field(s)",
                    cfg.types.len(),
                    cfg.statuses.len(),
                    cfg.fields.len()
                );
                match crate::store::Store::new(&cfg).load_lenient() {
                    Ok((items, bad)) => {
                        println!("items: {}", items.len());
                        if !bad.is_empty() {
                            println!("unreadable items: {}", bad.len());
                        }
                    }
                    Err(e) => println!("items: could not be read: {e:#}"),
                }
                println!("hooks: {} configured", cfg.hooks.count());
            }
        },
    }

    println!();
    println!("Report bugs to: {BUG_ADDRESS}");
    Ok(0)
}
