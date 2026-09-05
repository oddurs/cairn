// cairn — src/cmd/init.rs
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

    /// How much schema to start from
    #[arg(long, value_enum, default_value = "standard")]
    pub preset: Preset,

    /// Do not create the example item
    #[arg(long, action = ArgAction::SetTrue)]
    pub bare: bool,

    /// Overwrite an existing cairn.toml
    #[arg(long, action = ArgAction::SetTrue)]
    pub force: bool,
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
    if config_path.exists() && !args.force {
        bail!(
            "{} already exists (use --force to overwrite)",
            config_path.display()
        );
    }

    let name = args.name.unwrap_or_else(|| {
        cwd.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project".into())
    });

    let template = match args.preset {
        Preset::Minimal => MINIMAL,
        Preset::Standard => STANDARD,
    };
    let toml = template
        .replace("{{name}}", &escape(&name))
        .replace("{{dir}}", &escape(&args.dir));
    crate::store::write_atomic(&config_path, toml.as_bytes())?;

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

    if !args.bare {
        let path = write_example(&cfg, &items_dir)?;
        println!(
            "{} {}",
            style::green("created"),
            path.strip_prefix(&cwd).unwrap_or(&path).display()
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

fn write_example(cfg: &Config, dir: &Path) -> Result<PathBuf> {
    let today = store::today();
    let kind = cfg
        .project
        .default_type
        .clone()
        .or_else(|| cfg.types.first().map(|t| t.name.clone()));
    let mut front = String::from("---\nid: 1\ntitle: Adopt cairn for the roadmap\n");
    if let Some(k) = kind {
        front.push_str(&format!("type: {k}\n"));
    }
    front.push_str(&format!("status: {}\n", cfg.initial_status()));
    if let Some(m) = cfg.milestones.first() {
        front.push_str(&format!("milestone: {}\n", m.name));
    }
    front.push_str(&format!("created: {today}\nupdated: {today}\n---\n"));
    front.push_str(EXAMPLE_BODY);

    let path = dir.join(format!(
        "{}-adopt-cairn-for-the-roadmap.md",
        cfg.format_id(1)
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
format = 1

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

[[milestone]]
name = "v0.1"
title = "First usable version"
due = "2026-12-01"
description = "Enough to dogfood in a real project."

[[milestone]]
name = "v1.0"
title = "Stable release"
due = "2027-03-01"
description = "Documented, tested, and safe to depend on."

[[milestone]]
name = "later"
title = "Someday"
description = "Good ideas without a date yet."

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

format = 1

[project]
name = "{{name}}"
dir = "{{dir}}"

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

[render]
target = "ROADMAP.md"
group_by = "milestone"
"####;
