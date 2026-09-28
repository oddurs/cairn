// cairn — splitting a prompt's numbered steps into sub-prompts.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// 0147's Approach was three numbered steps, each of which became separate work
// in another repository without ever becoming an item: none could be claimed,
// ticked or blocked on by itself, and the order between them lived in prose.
// This turns such a list into ordinary items, in that order, each one a
// sub-prompt of the item it came from.
use crate::Assign;
use crate::cmd::set::apply;
use crate::config::{Addressing, Config};
use crate::hooks;
use crate::item::Item;
use crate::lock::Lock;
use crate::store::{Store, today};
use crate::style;
use anyhow::{Result, bail};
use clap::ArgAction;

#[derive(clap::Args)]
pub struct Args {
    /// The item to split
    #[arg(value_name = "ID")]
    pub id: String,

    /// The section holding the numbered steps
    #[arg(long, value_name = "SECTION", default_value = "Approach")]
    pub from: String,

    /// Create the steps with no order between them
    #[arg(long, action = ArgAction::SetTrue)]
    pub parallel: bool,

    /// Show the items it would create, and write nothing
    #[arg(short = 'n', long, action = ArgAction::SetTrue)]
    pub dry_run: bool,

    /// Print only the new ids
    #[arg(short, long, action = ArgAction::SetTrue)]
    pub quiet: bool,
}

/// The note that records a split, and that marks an item as split already.
const SPLIT_INTO: &str = "Split into ";

pub fn run(args: Args) -> Result<i32> {
    let cfg = Config::discover()?;
    let store = Store::new(&cfg);
    let lock = Lock::acquire(&cfg)?;
    let mut items = store.load_all()?;
    let parent = store.find_ref(&args.id)?;
    let parent_ref = cfg.format_id(parent.id);

    if already_split(&parent) {
        bail!(
            "{parent_ref} has been split already — its notes say into what. Edit those items \
             rather than splitting again"
        );
    }
    let Some((start, end)) = parent.section_span(&args.from) else {
        bail!("{parent_ref} has no `{}` section to split", args.from);
    };
    let section: Vec<&str> = parent
        .body
        .lines()
        .skip(start + 1)
        .take(end - start - 1)
        .collect();
    let steps = steps(&section);
    if steps.len() < 2 {
        bail!(
            "{parent_ref}'s `{}` section has {} numbered step(s); splitting needs at least two, \
             written `1.`, `2.`, … at the start of a line",
            args.from,
            steps.len()
        );
    }

    // A field that rolls up by id, and can name the parent's type, is how this
    // project says one item is part of another: `part_of` in the standard preset.
    let part_of = cfg.all_ref_fields().into_iter().find(|f| {
        f.rollup
            && f.by == Addressing::Id
            && f.name != "depends_on"
            && f.target
                .as_deref()
                .is_none_or(|t| t == "*" || Some(t) == parent.kind())
    });

    let mut created: Vec<Item> = Vec::new();
    for (n, step) in steps.iter().enumerate() {
        let id = store.next_id(&items)?;
        let now = today();
        let mut child = Item {
            id,
            meta: Default::default(),
            body: String::new(),
            path: store.path_for(id, None, "step"),
            front: String::new(),
            eol: Default::default(),
        };
        child.meta.title = Some(title(step));
        child.meta.created = Some(now.clone());
        child.meta.updated = Some(now);
        if let Some(agent) = crate::store::acting_agent() {
            child.meta.created_by = Some(agent);
        }
        // The steps of a milestone are work, not milestones: a container's
        // children take the project's default type instead of its own.
        child.meta.kind = if cfg.is_container(parent.kind()) {
            cfg.project.default_type.clone()
        } else {
            parent.meta.kind.clone()
        };
        apply(
            &mut child,
            &cfg,
            "status",
            Assign::Set(cfg.initial_status().to_string()),
        )?;
        if let Some(m) = parent.milestone() {
            apply(&mut child, &cfg, "milestone", Assign::Set(m.to_string()))?;
        }
        // As `new` would file it: the schema's defaults first, then what the
        // parent carries that a child needs — its priority, and anything the
        // schema requires.
        for f in &cfg.fields {
            if let Some(default) = &f.default {
                apply(&mut child, &cfg, &f.name, Assign::Set(default.clone()))?;
            }
        }
        for f in cfg
            .fields
            .iter()
            .filter(|f| f.required || f.name == "priority")
        {
            if let Some(value) = parent.meta.extra.get(f.name.as_str()) {
                child
                    .meta
                    .extra
                    .insert(f.name.as_str().into(), value.clone());
            }
        }
        if let Some(f) = cfg
            .fields
            .iter()
            .find(|f| f.required && child.get(&f.name).is_missing())
        {
            bail!(
                "field `{}` is required, and {parent_ref} has no value for its steps to take; \
                 set it on {parent_ref} first",
                f.name
            );
        }
        if let Some(def) = &part_of {
            apply(&mut child, &cfg, &def.name, Assign::Set(parent_ref.clone()))?;
        }
        if !args.parallel
            && let Some(previous) = created.last()
        {
            child.meta.depends_on = vec![previous.id];
        }
        child.set_body(&format!(
            "Step {} of {parent_ref}, {}.\n\n{}\n\n## Acceptance criteria\n\n- [ ]\n",
            n + 1,
            parent.title(),
            step.trim()
        ));
        store.stamp_new(&mut child)?;
        items.push(child.clone());
        created.push(child);
    }

    let ids: Vec<String> = created.iter().map(|c| cfg.format_id(c.id)).collect();
    if args.dry_run {
        for c in &created {
            let after = c
                .meta
                .depends_on
                .first()
                .map(|d| format!("  {}", style::dim(&format!("after {}", cfg.format_id(*d)))))
                .unwrap_or_default();
            println!(
                "  {} {}{after}",
                style::bold(&cfg.format_id(c.id)),
                c.title()
            );
        }
        println!(
            "\n{} {} item(s) would be created from {parent_ref}'s `{}`",
            style::dim("dry run:"),
            created.len(),
            args.from
        );
        return Ok(0);
    }

    let mut parent = parent;
    for c in &created {
        if !parent.meta.depends_on.contains(&c.id) {
            parent.meta.depends_on.push(c.id);
        }
    }
    parent.append_note(
        &today(),
        &format!(
            "{SPLIT_INTO}{}, one per numbered step of its {}. It finishes when they have.",
            ids.join(", "),
            args.from
        ),
    );
    parent.touch(&today());
    // Every write proved against the backlog as it will be, before the first
    // one lands: a refusal halfway would leave children with no note on the
    // parent, and running it again would make them twice.
    for item in created.iter().chain(std::iter::once(&parent)) {
        crate::refs::validate_on_write_in(&cfg, &mut items, item)?;
    }
    for child in &created {
        child.save()?;
    }
    parent.save()?;
    drop(lock);

    for c in &created {
        hooks::item(&cfg, &store, hooks::Event::AfterCreate, c);
    }
    hooks::item(&cfg, &store, hooks::Event::AfterChange, &parent);

    for c in &created {
        if args.quiet {
            println!("{}", cfg.format_id(c.id));
        } else {
            println!(
                "{} {}  {}",
                style::green("created"),
                style::bold(&cfg.format_id(c.id)),
                c.title()
            );
        }
    }
    if !args.quiet {
        println!(
            "{} {parent_ref} now depends on {}",
            style::green("split:"),
            ids.join(", ")
        );
    }
    Ok(0)
}

/// Whether a note on the item records a split already. Only a note's text
/// counts: a paragraph that happens to begin "Split into" is not a record.
fn already_split(item: &Item) -> bool {
    let lines: Vec<&str> = item.body.lines().collect();
    crate::item::headings(&item.body)
        .iter()
        .filter(|(_, _, text)| crate::item::is_note_heading(text))
        .any(|(line, _, _)| {
            lines[line + 1..]
                .iter()
                .find(|l| !l.trim().is_empty())
                .is_some_and(|l| l.starts_with(SPLIT_INTO))
        })
}

/// The first list of top-level numbered steps in a section: a line starting
/// `1.` or `1)` at the margin begins one, and it runs until the next. The list
/// ends at the first paragraph back at the margin after a blank line; code
/// fences are code, so a numbered example inside one is not a step.
fn steps(lines: &[&str]) -> Vec<String> {
    let mut out: Vec<Vec<&str>> = Vec::new();
    let mut fence: Option<&str> = None;
    let mut blank = false;
    for line in lines {
        let trimmed = line.trim_start();
        if let Some(marker) = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m)) {
            match fence {
                Some(open) if open == marker => fence = None,
                None => fence = Some(marker),
                Some(_) => {}
            }
        } else if fence.is_none() {
            if let Some(text) = numbered(line) {
                out.push(vec![text]);
                blank = false;
                continue;
            }
            if !out.is_empty() && blank && !line.trim().is_empty() && !line.starts_with(' ') {
                break;
            }
        }
        blank = line.trim().is_empty();
        if let Some(step) = out.last_mut() {
            step.push(line);
        }
    }
    out.into_iter()
        .map(|step| {
            step.iter()
                .map(|l| l.strip_prefix("   ").unwrap_or(l))
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        })
        .collect()
}

fn numbered(line: &str) -> Option<&str> {
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = &line[digits..];
    rest.strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))
        .map(str::trim)
}

/// A step's first sentence, without Markdown emphasis, as a title. The
/// sentence can run across the lines a step was wrapped onto.
fn title(step: &str) -> String {
    let first = step
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    let plain: String = first.replace("**", "").replace("__", "").replace('`', "");
    let sentence = [". ", ": ", "; "]
        .iter()
        .filter_map(|sep| plain.find(sep))
        .min()
        .map_or(plain.as_str(), |i| &plain[..i]);
    let sentence = sentence.trim().trim_end_matches(['.', ':', ';']).trim();
    if sentence.chars().count() <= 90 {
        return sentence.to_string();
    }
    let cut: String = sentence.chars().take(90).collect();
    match cut.rfind(' ') {
        Some(i) => format!("{}…", &cut[..i]),
        None => format!("{cut}…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_runs_until_the_next_one_at_the_margin() {
        let lines = [
            "Before the list.",
            "1. First: do this.",
            "   More about the first.",
            "",
            "2) Second step",
            "   - a detail",
            "10. Tenth",
        ];
        assert_eq!(
            steps(&lines),
            [
                "First: do this.\nMore about the first.",
                "Second step\n- a detail",
                "Tenth"
            ]
        );
    }

    #[test]
    fn a_fenced_example_and_the_paragraph_after_the_list_are_not_steps() {
        let lines = [
            "1. One",
            "   ```",
            "   2. not a step",
            "   ```",
            "2. Two",
            "",
            "After the list, a closing paragraph.",
            "",
            "1. A second list",
        ];
        assert_eq!(steps(&lines), ["One\n```\n2. not a step\n```", "Two"]);
    }

    #[test]
    fn a_title_is_the_first_sentence_without_emphasis() {
        assert_eq!(
            title("In the harrow repository, file and do the reader change: numeric `id`"),
            "In the harrow repository, file and do the reader change"
        );
        assert_eq!(
            title("**Install** the matching pair."),
            "Install the matching pair"
        );
        assert_eq!(
            title(
                "Migrate harrow, rim and nun, each in its own commit, verifying restored\nnumbers first. Then more."
            ),
            "Migrate harrow, rim and nun, each in its own commit, verifying restored numbers first"
        );
        let long = "word ".repeat(40);
        assert!(title(&long).chars().count() <= 91);
    }
}
