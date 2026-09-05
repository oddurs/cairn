// cairn — src/cmd/misc.rs
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
// cairn config / agent / completions / man.
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
    if args.json {
        println!("{}", serde_json::to_string_pretty(&schema_json(&cfg))?);
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
            format!(
                "{:<12} {}",
                s.name,
                style::dim(&format!("category = {}", s.category.as_str()))
            )
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
        cfg.milestones_ordered().iter().map(|m| {
            format!(
                "{:<12} {}",
                m.name,
                style::dim(&{
                    let mut bits: Vec<String> = Vec::new();
                    if let Some(t) = &m.title {
                        bits.push(t.clone());
                    }
                    if let Some(d) = &m.due {
                        bits.push(format!("due {d}"));
                    }
                    if let Some(st) = &m.status {
                        bits.push(format!("[{st}]"));
                    }
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
        FieldKind::Date => "date (YYYY-MM-DD)".into(),
        FieldKind::Number => "number".into(),
        FieldKind::Bool => "true / false".into(),
    };
    style::dim(&kind)
}

pub fn schema_json(cfg: &Config) -> serde_json::Value {
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
        },
        "types": cfg.types.iter().map(|t| json!({
            "name": t.name, "label": t.label, "description": t.description
        })).collect::<Vec<_>>(),
        "statuses": cfg.statuses.iter().map(|s| json!({
            "name": s.name, "label": s.label, "category": s.category.as_str(), "board": s.board
        })).collect::<Vec<_>>(),
        "fields": cfg.fields.iter().map(|f| json!({
            "name": f.name,
            "kind": format!("{:?}", f.kind).to_lowercase(),
            "values": f.values,
            "required": f.required,
            "default": f.default,
            "description": f.description,
        })).collect::<Vec<_>>(),
        "milestones": cfg.milestones_ordered().iter().map(|m| json!({
            "name": m.name, "title": m.title, "due": m.due,
            "description": m.description, "status": m.status
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
    let block = agent_block(&cfg);

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
fn agent_block(cfg: &Config) -> String {
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
            .map(|st| format!("`{}` ({})", st.name, st.category.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    for f in &cfg.fields {
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
            },
            if f.required { " (required)" } else { "" },
            f.description
                .as_ref()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default(),
        ));
    }
    if !cfg.milestones.is_empty() {
        s.push_str(&format!(
            "- **Milestones**: {}\n",
            cfg.milestones_ordered()
                .iter()
                .map(|m| match &m.due {
                    Some(d) => format!("`{}` (due {d})", m.name),
                    None => format!("`{}`", m.name),
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
