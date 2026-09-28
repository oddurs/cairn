// cairn — an item compiled into the prompt an agent reads.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// An item reads as a prompt that outlives the session (0154): its Problem is
// context, its Approach an instruction, its criteria a definition of done and
// its dated notes what earlier runs learned. What an agent needs is more than
// the item, though. It needs what the work rests on — the outcome it serves,
// what its dependencies concluded — and every agent used to assemble that by
// hand, differently, or not at all.
//
// This assembles it. A view, like `render`: nothing is written, and the file
// stays the source.
use crate::config::Config;
use crate::item::Item;
use crate::store::Store;
use anyhow::{Result, anyhow};
use clap::ArgAction;
use serde::Serialize;
use std::collections::HashSet;

#[derive(clap::Args)]
pub struct Args {
    /// Item id
    #[arg(value_name = "ID")]
    pub id: String,

    /// The layers as data rather than as text
    #[arg(long, action = ArgAction::SetTrue)]
    pub json: bool,
}

/// One part of the prompt, and where it came from, so a reader can check it.
#[derive(Serialize)]
pub struct Layer {
    pub name: &'static str,
    pub source: String,
    pub text: String,
}

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let items = Store::new(&cfg).load_for_reading()?;
    let item = find(&cfg, &items, &args.id)?;
    let layers = compile(&cfg, &items, item);
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "id": item.id,
                "ref": cfg.format_id(item.id),
                "title": item.title(),
                "layers": layers,
            }))?
        );
    } else {
        print!("{}", text(&cfg, item, &layers));
    }
    Ok(0)
}

/// An item by id, or by key as `show` accepts it: an agent names a milestone
/// by the key it sees everywhere else.
pub fn find<'a>(cfg: &Config, items: &'a [Item], raw: &str) -> Result<&'a Item> {
    let wanted = raw.trim();
    if let Some(item) = items
        .iter()
        .find(|i| i.key().is_some_and(|k| k.eq_ignore_ascii_case(wanted)))
    {
        return Ok(item);
    }
    let id = cfg.parse_id(raw)?;
    items
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| anyhow!("no item with id {}", cfg.format_id(id)))
}

/// The layers, in the order a model should read them: where it is, what it is
/// for, what it rests on, what to do, how to know it is done, what earlier runs
/// learned, and what to leave behind.
pub fn compile(cfg: &Config, items: &[Item], item: &Item) -> Vec<Layer> {
    let id = cfg.format_id(item.id);
    let parts = Parts::of(cfg, item);
    let mut layers = vec![project(cfg, item)];

    let above = ancestors(cfg, items, item);
    if !above.is_empty() {
        layers.push(Layer {
            name: "The outcome",
            source: above
                .iter()
                .map(|i| name_of(cfg, i))
                .collect::<Vec<_>>()
                .join(", "),
            text: above
                .iter()
                .map(|i| {
                    let lead = first_paragraph(i);
                    if lead.is_empty() {
                        format!("{} — {}", name_of(cfg, i), i.title())
                    } else {
                        format!("{} — {}\n{lead}", name_of(cfg, i), i.title())
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
        });
    }

    if !item.meta.depends_on.is_empty() {
        layers.push(Layer {
            name: "What this builds on",
            source: format!(
                "depends_on: {}",
                item.meta
                    .depends_on
                    .iter()
                    .map(|d| cfg.format_id(*d))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            text: item
                .meta
                .depends_on
                .iter()
                .map(|d| dependency(cfg, items, *d))
                .collect::<Vec<_>>()
                .join("\n\n"),
        });
    }

    // The title heads the whole prompt, so the task starts at the body.
    let task = parts.task.trim();
    layers.push(Layer {
        name: "The task",
        source: id.clone(),
        text: if task.is_empty() {
            item.title().to_string()
        } else {
            task.to_string()
        },
    });

    let criteria = item.criteria_list(cfg.project.criteria_section.as_deref());
    if !criteria.is_empty() {
        let mut open: Vec<String> = criteria
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.ticked)
            .map(|(n, c)| format!("{}. {}", n + 1, c.text))
            .collect();
        let done: Vec<String> = criteria
            .iter()
            .filter(|c| c.ticked)
            .map(|c| format!("- {}", c.text))
            .collect();
        if open.is_empty() {
            open.push("Every criterion is ticked.".into());
        }
        let mut text = open.join("\n");
        if !done.is_empty() {
            text += &format!("\n\nAlready true:\n{}", done.join("\n"));
        }
        layers.push(Layer {
            name: "Done when",
            source: format!("{id}'s acceptance criteria, numbered as `cairn tick` numbers them"),
            text,
        });
    }

    if !parts.notes.is_empty() {
        layers.push(Layer {
            name: "What earlier runs learned",
            source: format!("{id}'s notes, oldest first"),
            text: parts
                .notes
                .iter()
                .map(|(heading, text)| format!("## {heading}\n\n{text}"))
                .collect::<Vec<_>>()
                .join("\n\n"),
        });
    }

    layers.push(Layer {
        name: "When you stop",
        source: "cairn".into(),
        text: format!(
            "- Record what you learn as you go: `cairn note {id} \"…\"`.\n\
             - Tick each criterion as it becomes true, with the evidence in a note: \
             `cairn tick {id} <n>`.\n\
             - When it is done: `cairn close {id} --result \"<what it concluded>\"`.\n\
             - To hand it back: `cairn release {id} --reason \"<what the next taker needs>\"`."
        ),
    });
    layers
}

/// The layers as Markdown, ready to hand to a model. Every heading a layer
/// carries — an item's own `## Problem`, a note's date — sits below the
/// layer's, so the outline a model reads is the prompt's and not the file's.
pub fn text(cfg: &Config, item: &Item, layers: &[Layer]) -> String {
    let mut out = format!("# {} {}\n", cfg.format_id(item.id), item.title());
    for layer in layers {
        out += &format!(
            "\n## {}\n\n_From {}._\n\n{}\n",
            layer.name,
            layer.source,
            crate::item::beneath(layer.text.trim_end(), 2)
        );
    }
    out
}

fn project(cfg: &Config, item: &Item) -> Layer {
    let id = cfg.format_id(item.id);
    let mut text = match &cfg.project.description {
        Some(d) if !d.trim().is_empty() => format!("{}: {}", cfg.project.name, d.trim()),
        _ => cfg.project.name.clone(),
    };
    text += &format!(
        "\n\nWork is tracked as items under `{}`. Claim an item before working on it \
         (`cairn claim {id}`), so nobody duplicates the work. `cairn check` must pass \
         before you report finished.",
        cfg.project.dir
    );
    match item.meta.assignee.as_deref().filter(|a| !a.is_empty()) {
        Some(who) => {
            text += &format!(
                "\n\n{id} is {} and claimed by {who}{}.",
                item.status(),
                item.meta
                    .claimed
                    .as_deref()
                    .map(|d| format!(" since {d}"))
                    .unwrap_or_default()
            );
        }
        None => text += &format!("\n\n{id} is {}, and nobody has claimed it.", item.status()),
    }
    Layer {
        name: "How this project works",
        source: "cairn.toml, and the item's state".into(),
        text,
    }
}

/// Everything an item is filed under, through any field that rolls up —
/// its milestone, a parent — outermost first.
fn ancestors<'a>(cfg: &Config, items: &'a [Item], item: &Item) -> Vec<&'a Item> {
    let rollups: Vec<_> = cfg
        .all_ref_fields()
        .into_iter()
        .filter(|f| f.rollup)
        .collect();
    let mut seen = HashSet::from([item.id]);
    let mut found: Vec<&Item> = Vec::new();
    let mut frontier: Vec<&Item> = items.iter().filter(|i| i.id == item.id).collect();
    // Bounded, so a cycle slipped in by hand cannot make this run forever.
    for _ in 0..8 {
        let mut next = Vec::new();
        for current in &frontier {
            for def in &rollups {
                for parent in crate::refs::targets(items, current, def) {
                    if seen.insert(parent.id) {
                        next.push(parent);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        found.extend(next.iter().copied());
        frontier = next;
    }
    found.reverse();
    found
}

fn dependency(cfg: &Config, items: &[Item], id: crate::identity::Id) -> String {
    let Some(dep) = items.iter().find(|i| i.id == id) else {
        return format!("{} — not found in this backlog.", cfg.format_id(id));
    };
    let head = format!(
        "{} — {} ({})",
        cfg.format_id(dep.id),
        dep.title(),
        dep.status()
    );
    if !cfg.category(dep.status()).is_closed() {
        return format!("{head}\nNot finished: this item is blocked until it is.");
    }
    if let Some(result) = dep.result() {
        return format!("{head}\nResult: {result}");
    }
    match Parts::of(cfg, dep).notes.last() {
        Some((heading, text)) => {
            format!("{head}\nNo result was recorded. Its last note, {heading}:\n{text}")
        }
        None => format!("{head}\nFinished without a result or a note."),
    }
}

/// How an item is referred to: a container by its key, anything else by its id.
fn name_of(cfg: &Config, item: &Item) -> String {
    match (item.key(), cfg.is_container(item.kind())) {
        (Some(key), true) => format!("{} {key}", item.kind().unwrap_or("item")),
        _ => cfg.format_id(item.id),
    }
}

/// The first paragraph of prose in a body, skipping headings.
fn first_paragraph(item: &Item) -> String {
    let mut out = Vec::new();
    for line in item.body.lines() {
        let blank = line.trim().is_empty();
        if line.trim_start().starts_with('#') || (blank && out.is_empty()) {
            if out.is_empty() {
                continue;
            }
            break;
        }
        if blank {
            break;
        }
        out.push(line.trim_end());
    }
    out.join("\n")
}

/// An item's body, separated into what to do and what earlier runs learned.
/// Criteria and the Result are left out of the task: the one has its own
/// layer, and the other is for the work that comes after.
struct Parts {
    task: String,
    /// Each note's heading and text, oldest first.
    notes: Vec<(String, String)>,
}

impl Parts {
    fn of(cfg: &Config, item: &Item) -> Parts {
        let lines: Vec<&str> = item.body.lines().collect();
        let marks = crate::item::headings(&item.body);
        let top = marks.iter().map(|(_, level, _)| *level).min();
        // Sections split at the shallowest level, and at every note however
        // deep it sits: a body under one `# Context` still has notes after it.
        let starts: Vec<(usize, &str)> = marks
            .iter()
            .filter(|(_, level, text)| Some(*level) == top || crate::item::is_note_heading(text))
            .map(|(line, _, text)| (*line, text.as_str()))
            .collect();
        // Exactly the lines Done when shows, and no others: a checklist outside
        // a configured criteria section is part of the task.
        let criteria: HashSet<usize> = item
            .criteria_list(cfg.project.criteria_section.as_deref())
            .iter()
            .map(|c| c.line)
            .collect();
        let keep = |range: std::ops::Range<usize>| -> Vec<&str> {
            range
                .filter(|n| !criteria.contains(n))
                .map(|n| lines[n])
                .collect()
        };

        let mut task = keep(0..starts.first().map_or(lines.len(), |(l, _)| *l));
        let mut notes = Vec::new();
        for (n, (start, heading)) in starts.iter().enumerate() {
            let end = starts.get(n + 1).map_or(lines.len(), |(l, _)| *l);
            if crate::item::is_note_heading(heading) {
                notes.push((
                    (*heading).to_string(),
                    lines[start + 1..end].join("\n").trim().to_string(),
                ));
            } else if !heading.eq_ignore_ascii_case("result") {
                let section = keep(*start..end);
                // A section that held nothing but criteria has nothing left to
                // say; one with prose beside its boxes keeps the prose.
                if section[1..].iter().any(|l| !l.trim().is_empty()) {
                    task.extend(section);
                }
            }
        }
        Parts {
            task: task.join("\n").trim().to_string(),
            notes,
        }
    }
}
