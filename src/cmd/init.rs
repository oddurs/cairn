// cairn — src/cmd/init.rs
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
// cairn init — write the schema and the item directory.
use crate::config::{CONFIG_FILE, Config};
use crate::store;
use crate::style;
use anyhow::{Result, bail};
use clap::{ArgAction, ValueEnum};
use std::path::{Path, PathBuf};

#[derive(clap::Args)]
pub struct Args {
    /// Project name (defaults to the directory name)
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,

    /// Where item files live, relative to cairn.toml
    #[arg(long, value_name = "DIR", default_value = "cairn/items")]
    pub dir: String,

    /// How much schema to start from [default: standard]
    #[arg(long, value_enum, conflicts_with = "from")]
    pub preset: Option<Preset>,

    /// Start from another cairn project's schema instead of a preset
    #[arg(long, value_name = "PATH")]
    pub from: Option<PathBuf>,

    /// Do not create the example item
    #[arg(long, action = ArgAction::SetTrue)]
    pub bare: bool,

    /// Overwrite an existing cairn.toml
    #[arg(long, action = ArgAction::SetTrue)]
    pub force: bool,

    /// Teach git to resolve the files cairn generates. Safe to run on an
    /// existing project, which is how one adopts it.
    #[arg(long, action = ArgAction::SetTrue)]
    pub git: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum Preset {
    /// Two statuses, no custom fields — grow it as you go
    Minimal,
    /// Types, priorities, milestones and saved views
    Standard,
}

pub fn run(args: Args) -> Result<i32> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join(CONFIG_FILE);

    // `--git` on a project that already exists installs only the integration.
    // Adopting it should not mean re-running init over a live backlog.
    if config_path.exists() && !args.force {
        if args.git {
            let cfg = Config::discover()?;
            return crate::cmd::git::setup(&cfg).map(|()| 0);
        }
        bail!(
            "{} already exists (use --force to overwrite, or --git to add the \
             git integration to it)",
            config_path.display()
        );
    }

    let name = args.name.unwrap_or_else(|| {
        cwd.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project".into())
    });

    let toml = match &args.from {
        Some(path) => adopt(path, &name, &args.dir)?,
        None => {
            let template = match args.preset.unwrap_or(Preset::Standard) {
                Preset::Minimal => MINIMAL,
                Preset::Standard => STANDARD,
            };
            template
                .replace("{{name}}", &escape(&name))
                .replace("{{dir}}", &escape(&args.dir))
        }
    };
    crate::store::write_atomic(&config_path, toml.as_bytes())?;
    if let Some(path) = &args.from {
        println!(
            "{} {}",
            style::dim("schema from"),
            style::dim(&path.display().to_string())
        );
    }

    let items_dir = cwd.join(&args.dir);
    std::fs::create_dir_all(&items_dir)?;

    // Without this, a checkout with core.autocrlf=true rewrites every item on
    // the way in and cairn faithfully writes CRLFs back, so the whole backlog
    // churns whenever it crosses platforms. Pinning the item directory to LF
    // gives the repository one answer regardless of client configuration.
    // The lock is transient and lives beside the items, so it is ignored here
    // rather than in the project's own .gitignore, which is not cairn's to edit.
    let ignore = items_dir.join(".gitignore");
    if !ignore.exists() {
        crate::store::write_atomic(&ignore, b"# cairn's transient write lock.\n.lock\n")?;
    }

    let attributes = items_dir.join(".gitattributes");
    if !attributes.exists() {
        crate::store::write_atomic(
            &attributes,
            b"# Item files are LF regardless of platform, so they do not churn\n\
              # when a repository is shared across systems.\n\
              *.md text eol=lf\n",
        )?;
    }

    // Load it back so the example item is written against the real schema
    // rather than assumptions about it.
    let cfg = Config::load(&config_path)?;

    println!("{} {}", style::green("created"), style::bold(CONFIG_FILE));
    println!("{} {}/", style::green("created"), args.dir);

    // A milestone is an item now, so a project that wants a roadmap needs some.
    // `--bare` gets none, for the same reason it gets no example item: it asked
    // for the schema and nothing else.
    // An adopted schema need not have milestones in it at all, and writing
    // items of a type it does not declare would hand somebody a new project
    // that fails its own `cairn check`.
    let has_milestones = cfg.schedule_type().is_some();
    if !args.bare && has_milestones {
        for (n, (key, title, due, why)) in [
            (
                "v0.1",
                "First usable version",
                Some("2026-12-01"),
                "Enough to dogfood in a real project.",
            ),
            (
                "v1.0",
                "Stable release",
                Some("2027-03-01"),
                "Documented, tested, and safe to depend on.",
            ),
            ("later", "Someday", None, "Good ideas without a date yet."),
        ]
        .into_iter()
        .enumerate()
        {
            let path = write_milestone(&cfg, &items_dir, n as u32 + 1, key, title, due, why)?;
            println!(
                "{} {}",
                style::green("created"),
                path.strip_prefix(&cwd).unwrap_or(&path).display()
            );
        }
    }

    if !args.bare {
        // After the milestones, and pointing at the first of them.
        let (id, milestone) = if has_milestones {
            (4, Some("v0.1"))
        } else {
            (1, None)
        };
        let path = write_example(&cfg, &items_dir, id, milestone)?;
        println!(
            "{} {}",
            style::green("created"),
            path.strip_prefix(&cwd).unwrap_or(&path).display()
        );
    }

    if args.git {
        println!();
        crate::cmd::git::setup(&cfg)?;
    } else if crate::cmd::git::in_repository(&cwd) {
        println!();
        println!(
            "{}",
            style::dim(
                "Tip: `cairn init --git` teaches git to resolve the roadmap and renumber\n                      colliding ids when branches merge."
            )
        );
    }

    println!();
    println!("{}", style::bold("Next:"));
    println!("  cairn new \"Ship the first release\" --milestone v0.1");
    println!("  cairn board");
    println!("  cairn render                 # writes ROADMAP.md");
    println!("  cairn agent --write AGENTS.md  # teach coding agents the schema");
    Ok(0)
}

fn write_example(cfg: &Config, dir: &Path, id: u32, milestone: Option<&str>) -> Result<PathBuf> {
    let today = store::today();
    let kind = cfg
        .project
        .default_type
        .clone()
        .or_else(|| cfg.types.first().map(|t| t.name.clone()));
    let mut front = format!("---\nid: {id}\ntitle: Adopt cairn for the roadmap\n");
    if let Some(k) = kind {
        front.push_str(&format!("type: {k}\n"));
    }
    front.push_str(&format!("status: {}\n", cfg.initial_status()));
    if let Some(m) = milestone {
        front.push_str(&format!("milestone: {m}\n"));
    }
    front.push_str(&format!("created: {today}\nupdated: {today}\n---\n"));
    front.push_str(EXAMPLE_BODY);

    let path = dir.join(format!(
        "{}-adopt-cairn-for-the-roadmap.md",
        cfg.format_id(id)
    ));
    crate::store::write_atomic(&path, front.as_bytes())?;
    Ok(path)
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

const EXAMPLE_BODY: &str = r#"
This is a cairn item: a Markdown file with YAML frontmatter. Edit it by hand,
or from the command line:

    cairn set 1 status=done
    cairn show 1
    cairn list --status done

Delete this file once you have the hang of it.

## Acceptance criteria

- [ ] `cairn.toml` describes the workflow this project actually uses
- [ ] `cairn render` produces a ROADMAP.md worth linking from the README
- [ ] `cairn check` passes in CI
"#;

const STANDARD: &str = r####"# cairn.toml — the schema for this project's roadmap and issues.
#
# Everything here is configurable: the item types, the statuses they move
# through, any extra fields you want to track, the milestones, the saved views,
# and how ROADMAP.md gets rendered. Delete what you do not need.
#
#   cairn config          show the resolved schema
#   cairn check           validate every item against it
#   man cairn             full reference

# On-disk format version. cairn refuses to open a project written in a format it
# does not know, rather than misreading it. See "Compatibility" in the manual.
format = 3

[project]
name = "{{name}}"
dir = "{{dir}}"           # where item files live, relative to this file
id_width = 4              # 0001, 0002, ...
default_type = "feature"
default_status = "backlog"
# filename_max = 255      # longest filename your filesystem accepts, in bytes;
                          # titles are only shortened as much as this demands
# url = "https://github.com/OWNER/REPO/blob/main"   # makes rendered items clickable

# ─── Item types ──────────────────────────────────────────────────────────────
# `template` seeds the body of new items of that type, so every bug report and
# every feature arrives with the same headings.

[[type]]
name = "feature"
icon = "+"
color = "cyan"
template = """
## Problem

## Proposal

## Acceptance criteria

- [ ]
"""

[[type]]
name = "bug"
icon = "!"
color = "red"
template = """
## What happens

## What should happen

## Reproduction

1.
"""

[[type]]
name = "chore"
icon = "~"
color = "gray"

[[type]]
name = "docs"
icon = "*"
color = "blue"

# ─── Statuses ────────────────────────────────────────────────────────────────
# Order matters: it is the column order on the board and the sort order in
# listings. `category` is what the tool reasons about — open / active / done /
# dropped — so you can rename the statuses themselves to anything you like.

[[status]]
name = "backlog"
category = "open"
color = "gray"

[[status]]
name = "planned"
category = "open"
color = "blue"

[[status]]
name = "doing"
label = "in progress"
category = "active"
color = "yellow"

[[status]]
name = "blocked"
category = "active"
color = "red"

[[status]]
name = "done"
category = "done"
color = "green"

[[status]]
name = "dropped"
category = "dropped"
color = "gray"
board = false             # hide this column on `cairn board`

# ─── Custom fields ───────────────────────────────────────────────────────────
# kind = text | enum | list | date | number | bool
# `column = true` puts the field in the default `cairn list` table.
# Composition. `part_of` is a reference: its values name other items rather than
# describing this one. Many-valued on purpose — an item can belong to two larger
# efforts at once, which a `parent` field could not express and which is why
# there is no such field.
#
# Nothing requires it. `cairn new` still takes only a title, and structure is
# added afterwards: `cairn set 12 part_of=7`.
[[type]]
name = "milestone"
# `groups` is what makes a type the thing work is filed *under* rather than work
# itself: absent from `cairn next`, absent from the board, and accumulating the
# progress of everything filed under it. Declaring it also creates the field, so
# `milestone: v0.1` names one by its key — a handle, not an identity, since `id`
# is still the number.
#
# `one` because work ships in exactly one release. A type that groups work
# several ways at once — an epic, a theme — says `many`.
groups = "one"
inverse = "scheduled"
description = "a release, or whatever this project ships"

[[field]]
name = "due"
kind = "date"
description = "when a milestone is meant to land"

[[field]]
name = "part_of"
kind = "ref"
target = "*"
cardinality = "many"
acyclic = true
rollup = true
inverse = "contains"
description = "a larger piece of work this belongs to"

# Enum values are ordered: `cairn list --sort priority` respects that order.

[[field]]
name = "priority"
kind = "enum"
values = ["p0", "p1", "p2", "p3"]
default = "p2"
column = true
description = "p0 is a release blocker"

[[field]]
name = "effort"
kind = "enum"
values = ["s", "m", "l", "xl"]
description = "Rough size, not an estimate"

[[field]]
name = "area"
kind = "text"
description = "Subsystem this touches"

# ─── Milestones ──────────────────────────────────────────────────────────────
# Sections of the rendered roadmap, in due-date order.

# ─── Saved views ─────────────────────────────────────────────────────────────
# `cairn list --view next`, `cairn board --view triage`

[[view]]
name = "now"
description = "What is actually being worked on"
filter = "category=active"
sort = "priority,id"

[[view]]
name = "next"
description = "Planned work for the nearest milestone"
filter = "status=planned"
sort = "priority,milestone"

[[view]]
name = "triage"
description = "Items that still need a milestone or a priority"
filter = "milestone=,category!=done"
columns = ["id", "type", "title", "created"]

# ─── Hooks ───────────────────────────────────────────────────────────────────
# cairn does not embed a scripting language; it runs yours. A hook takes one of
# two forms:
#
#   a string  runs through the platform shell — convenient, and therefore
#             platform-specific ($VAR on Unix is %VAR% under cmd.exe)
#   an array  is executed directly with no shell at all — portable
#
# Each runs from the project root with the event in the environment and the
# full item as JSON on stdin:
#
#   CAIRN_EVENT CAIRN_ROOT CAIRN_CONFIG
#   CAIRN_ITEM_ID CAIRN_ITEM_PATH CAIRN_ITEM_TITLE
#   CAIRN_ITEM_STATUS CAIRN_ITEM_TYPE CAIRN_ITEM_MILESTONE CAIRN_ITEM_CATEGORY
#
# Hooks run after the change is on disk, so a failing hook warns but never rolls
# anything back. Suppress them with --no-hooks or CAIRN_NO_HOOKS=1.

# Keeping the rendered roadmap current is the tool's whole point, so it is done
# for you. Comment these out if you would rather render by hand — but a roadmap
# that drifts from the items is the rot cairn exists to prevent, and leaving it
# to discipline is how it starts.

[hooks]
after-create = "cairn render -q"
after-change = "cairn render -q"
after-remove = "cairn render -q"
# after-render = "git add ROADMAP.md"
# after-change = ["python3", "scripts/notify.py"]   # portable form

# ─── Rendered roadmap ────────────────────────────────────────────────────────

[render]
target = "ROADMAP.md"
title = "Roadmap"
group_by = "milestone"    # any field works: milestone, area, assignee, type
include = "category!=dropped"
checkbox = true           # render items as a task list
show_ids = true
progress = true           # per-milestone progress bar
group_by_status = true    # sub-group each section by status
link_items = false        # requires project.url
# header = "docs/roadmap-intro.md"   # spliced in above the generated body
# footer = "docs/roadmap-outro.md"
"####;

const MINIMAL: &str = r####"# cairn.toml — roadmap and issue schema.
# Start here and add types, fields, milestones and views as you need them.
# See `cairn init --preset standard` for a fully commented example.

format = 3

[project]
name = "{{name}}"
dir = "{{dir}}"

# Order matters: it is the column order on `cairn board` and the sort order in
# listings, so reordering these blocks changes behaviour.
[[status]]
name = "todo"
category = "open"

[[status]]
name = "doing"
category = "active"
color = "yellow"

[[status]]
name = "done"
category = "done"
color = "green"

# `groups` makes this the thing work is filed under, and creates the field that
# names one: `milestone: v0.1`.
[[type]]
name = "milestone"
groups = "one"

[[field]]
name = "due"
kind = "date"

[render]
target = "ROADMAP.md"
group_by = "milestone"
"####;

/// Write one of the milestones a new project starts with.
///
/// They are ordinary items — the type and the reference are declared in the
/// configuration like anything else — and they depend on each other in
/// sequence, because a roadmap is a sequence and that is now how the order is
/// expressed rather than by the order of blocks in a file.
#[allow(clippy::too_many_arguments)]
fn write_milestone(
    cfg: &Config,
    items_dir: &std::path::Path,
    id: u32,
    key: &str,
    title: &str,
    due: Option<&str>,
    body: &str,
) -> Result<std::path::PathBuf> {
    let path = items_dir.join(cfg.filename_for(id, title));
    let mut item = crate::item::Item {
        id,
        meta: Default::default(),
        body: String::new(),
        path: path.clone(),
        front: String::new(),
        eol: Default::default(),
    };
    item.meta.title = Some(title.to_string());
    item.meta.key = Some(key.to_string());
    item.meta.kind = Some(
        cfg.schedule_type()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "milestone".into()),
    );
    item.meta.status = Some(cfg.initial_status().to_string());
    item.meta.created = Some(crate::store::today());
    if id > 1 {
        item.meta.depends_on = vec![id - 1];
    }
    if let Some(d) = due {
        item.set_extra("due", Some(crate::item::Field::Text(d.to_string())));
    }
    item.set_body(body);
    item.save()?;
    Ok(path)
}

/// Start from another project's schema.
///
/// A copy, and deliberately not an include. An `extends` key would end
/// `cairn.toml` being the whole truth about a project: reading it would mean
/// resolving a path that may be outside the repository, may not exist on a
/// clone, and may have changed since. Somebody who clones a repository can
/// understand its backlog, and that is worth more than saving a copy.
///
/// The cost is real — the two files drift — and it is the right cost. A schema
/// two projects share is two projects that cannot change independently.
///
/// `toml_edit` rather than a parse-and-print, because the comments are half of
/// what makes a schema legible and a round trip through serde would drop every
/// one of them.
fn adopt(from: &Path, name: &str, dir: &str) -> Result<String> {
    let source = if from.is_dir() {
        from.join(CONFIG_FILE)
    } else {
        from.to_path_buf()
    };
    if !source.is_file() {
        bail!(
            "{} is not a cairn project ({} is not there)",
            from.display(),
            source.display()
        );
    }

    // Read it as a project first, so an unreadable or newer schema is refused
    // here rather than copied and refused later in the new project.
    let cfg = Config::load_for_migration(&source)?;
    if cfg.format() < crate::config::CURRENT_FORMAT {
        bail!(
            "{} is format {}, and a new project is written at format {}\n\
             run `cairn migrate` there first, then copy its schema",
            from.display(),
            cfg.format(),
            crate::config::CURRENT_FORMAT
        );
    }

    let text = std::fs::read_to_string(&source)?;
    let mut doc: toml_edit::DocumentMut = text.parse()?;

    // `[project]` is the one table that is about *this* project rather than
    // about the schema. Everything in it is either replaced or dropped: a name,
    // a description and a repository url belong to whoever wrote them.
    let project = doc
        .entry("project")
        .or_insert(toml_edit::Item::Table(Default::default()));
    if let Some(table) = project.as_table_mut() {
        table.insert("name", toml_edit::value(name));
        table.insert("dir", toml_edit::value(dir));
        for gone in ["description", "url"] {
            table.remove(gone);
        }
    }

    // The url went with the other project, so linking items would link them
    // nowhere. Turned off rather than left to warn: a project should not be
    // handed to somebody already failing its own check.
    let mut unlinked = false;
    if let Some(render) = doc.get_mut("render").and_then(|r| r.as_table_mut())
        && render
            .get("link_items")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        render.insert("link_items", toml_edit::value(false));
        unlinked = true;
    }
    if unlinked {
        println!(
            "{}",
            style::dim("render.link_items turned off — set project.url and turn it back on")
        );
    }

    Ok(doc.to_string())
}
