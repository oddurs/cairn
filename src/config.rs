// cairn — configuration: the schema the whole tool is driven by.
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
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "cairn.toml";

/// The on-disk format this build reads and writes.
///
/// Durability is a promise, and this is where it is kept. The rules, in full:
///
/// * A patch or minor release may add optional keys, and nothing else.
/// * Removing a key, changing its meaning, or making an optional key required
///   needs a new format number, a major release, and a migration.
/// * A reader preserves keys it does not recognise, so a project opened by an
///   older cairn is never silently stripped of data a newer one wrote.
/// * A project recording a format this build does not know is refused with an
///   explanation, rather than misread.
pub const CURRENT_FORMAT: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// On-disk format version. Absent means 1: projects created before the
    /// version existed are format 1 by definition.
    #[serde(default)]
    pub format: Option<u32>,
    #[serde(default)]
    pub project: Project,
    #[serde(default, rename = "type")]
    pub types: Vec<ItemType>,
    #[serde(default, rename = "status")]
    pub statuses: Vec<Status>,
    #[serde(default, rename = "field")]
    pub fields: Vec<FieldDef>,
    #[serde(default, rename = "milestone")]
    pub milestones: Vec<Milestone>,
    #[serde(default, rename = "view")]
    pub views: Vec<View>,
    #[serde(default)]
    pub render: RenderConfig,
    #[serde(default)]
    pub hooks: Hooks,

    /// Absolute path to the directory containing `cairn.toml`. Filled in at load
    /// time, not read from the file.
    #[serde(skip)]
    pub root: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    #[serde(default = "default_project_name")]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Directory (relative to root) holding item files.
    #[serde(default = "default_dir")]
    pub dir: String,
    /// Zero-padding width for item ids: 4 gives `0001`.
    #[serde(default = "default_id_width")]
    pub id_width: usize,
    #[serde(default)]
    pub default_type: Option<String>,
    #[serde(default)]
    pub default_status: Option<String>,
    /// Base URL for `render.link_items`, e.g. a GitHub blob URL.
    #[serde(default)]
    pub url: Option<String>,
    /// Longest filename the target filesystem accepts, in bytes. Item titles
    /// are only ever shortened to satisfy this. 255 is the POSIX-typical
    /// maximum; lower it for eCryptfs (143) or other constrained systems.
    #[serde(default = "default_filename_max")]
    pub filename_max: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemType {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Markdown skeleton inserted into the body of new items of this type.
    #[serde(default)]
    pub template: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    #[default]
    Open,
    Active,
    Done,
    Dropped,
}

impl Category {
    pub fn is_closed(self) -> bool {
        matches!(self, Category::Done | Category::Dropped)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Open => "open",
            Category::Active => "active",
            Category::Done => "done",
            Category::Dropped => "dropped",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    /// Drives "is this finished?" logic everywhere: progress bars, default
    /// filters, roadmap rendering.
    #[serde(default)]
    pub category: Category,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    /// Show this status as a column on `cairn board`.
    #[serde(default = "yes")]
    pub board: bool,
}

impl Status {
    pub fn display(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Text,
    Enum,
    List,
    Date,
    Number,
    Bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDef {
    pub name: String,
    #[serde(default = "default_kind")]
    pub kind: FieldKind,
    #[serde(default)]
    pub values: Vec<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Include in the default `cairn list` table.
    #[serde(default)]
    pub column: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Milestone {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    /// ISO date (YYYY-MM-DD). Milestones sort by this, undated ones last.
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Free-form: "shipped", "planned", … Only used for display and filtering.
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub filter: Option<String>,
    #[serde(default)]
    pub group_by: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub columns: Vec<String>,
}

/// One hook, in either of two forms.
///
/// A string goes through the platform shell, which is convenient and therefore
/// platform-specific: `$VAR` on a Unix shell is `%VAR%` under `cmd.exe`. An
/// array is executed directly with no shell at all, which is portable. Projects
/// that must run on more than one platform should use the array form.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Hook {
    /// `after-create = "cairn render -q"` — interpreted by the platform shell.
    Shell(String),
    /// `after-create = ["python3", "scripts/notify.py"]` — executed directly.
    Argv(Vec<String>),
}

/// Programs to run when things happen. The extension point: cairn does not
/// embed a scripting language, it runs yours.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hooks {
    /// Run after an item is created.
    #[serde(default, rename = "after-create")]
    pub after_create: Option<Hook>,
    /// Run after an item's fields change.
    #[serde(default, rename = "after-change")]
    pub after_change: Option<Hook>,
    /// Run after an item is deleted.
    #[serde(default, rename = "after-remove")]
    pub after_remove: Option<Hook>,
    /// Run after the roadmap file is written.
    #[serde(default, rename = "after-render")]
    pub after_render: Option<Hook>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderConfig {
    /// Output file, relative to root.
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Field to group sections by — usually `milestone`, but any field works.
    #[serde(default = "default_group_by")]
    pub group_by: String,
    /// Filter expression limiting which items are rendered.
    #[serde(default)]
    pub include: Option<String>,
    #[serde(default = "yes")]
    pub checkbox: bool,
    #[serde(default = "yes")]
    pub show_ids: bool,
    #[serde(default)]
    pub link_items: bool,
    #[serde(default = "yes")]
    pub progress: bool,
    #[serde(default = "yes")]
    pub group_by_status: bool,
    /// Markdown files spliced in above/below the generated body.
    #[serde(default)]
    pub header: Option<String>,
    #[serde(default)]
    pub footer: Option<String>,
}

fn yes() -> bool {
    true
}
fn default_project_name() -> String {
    "Project".into()
}
fn default_dir() -> String {
    "cairn/items".into()
}
fn default_id_width() -> usize {
    4
}
fn default_filename_max() -> usize {
    255
}
fn default_kind() -> FieldKind {
    FieldKind::Text
}
fn default_target() -> String {
    "ROADMAP.md".into()
}
fn default_group_by() -> String {
    "milestone".into()
}

impl Default for Project {
    fn default() -> Self {
        Project {
            name: default_project_name(),
            description: None,
            dir: default_dir(),
            id_width: default_id_width(),
            default_type: None,
            default_status: None,
            url: None,
            filename_max: default_filename_max(),
        }
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        RenderConfig {
            target: default_target(),
            title: None,
            group_by: default_group_by(),
            include: None,
            checkbox: true,
            show_ids: true,
            link_items: false,
            progress: true,
            group_by_status: true,
            header: None,
            footer: None,
        }
    }
}

impl Config {
    /// Walk up from `start` looking for `cairn.toml`.
    pub fn find(start: &Path) -> Option<PathBuf> {
        let mut dir = Some(start);
        while let Some(d) = dir {
            let candidate = d.join(CONFIG_FILE);
            if candidate.is_file() {
                return Some(candidate);
            }
            dir = d.parent();
        }
        None
    }

    pub fn load(path: &Path) -> Result<Config> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;

        // The format is read before anything else, so a project written by a
        // newer cairn is told apart from one that is simply malformed. Without
        // this, a key added in a later format would surface as "unknown field",
        // which sends the reader looking for a typo that is not there.
        Config::check_format(&text, path)?;

        let mut cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        cfg.root = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        cfg.validate()?;
        Ok(cfg)
    }

    /// Load by searching upward from the current directory.
    pub fn discover() -> Result<Config> {
        let cwd = std::env::current_dir().context("resolving current directory")?;
        match Config::find(&cwd) {
            Some(p) => Config::load(&p),
            None => bail!(
                "no {CONFIG_FILE} found in {} or any parent directory\n\
                 run `cairn init` to create one",
                cwd.display()
            ),
        }
    }

    /// Read just the format key, tolerating anything else in the file.
    fn check_format(text: &str, path: &Path) -> Result<()> {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            format: Option<u32>,
        }
        // A file too malformed to probe is left for the real parser to explain.
        let Ok(probe) = toml::from_str::<Probe>(text) else {
            return Ok(());
        };
        let found = probe.format.unwrap_or(1);
        if found > CURRENT_FORMAT {
            bail!(
                "{} is format {found}, and this cairn understands up to {CURRENT_FORMAT}\n\
                 it was written by a newer version — upgrade cairn to open it",
                path.display()
            );
        }
        Ok(())
    }

    /// The format this project records, defaulting to 1.
    pub fn format(&self) -> u32 {
        self.format.unwrap_or(1)
    }

    fn validate(&self) -> Result<()> {
        if self.statuses.is_empty() {
            bail!("{CONFIG_FILE}: at least one [[status]] must be defined");
        }
        check_unique("status", self.statuses.iter().map(|s| s.name.as_str()))?;
        check_unique("type", self.types.iter().map(|t| t.name.as_str()))?;
        check_unique("field", self.fields.iter().map(|f| f.name.as_str()))?;
        check_unique("milestone", self.milestones.iter().map(|m| m.name.as_str()))?;
        check_unique("view", self.views.iter().map(|v| v.name.as_str()))?;

        for f in &self.fields {
            if RESERVED_FIELDS.contains(&f.name.as_str()) {
                bail!(
                    "{CONFIG_FILE}: [[field]] name `{}` is reserved (built-in field)",
                    f.name
                );
            }
            if f.kind == FieldKind::Enum && f.values.is_empty() {
                bail!(
                    "{CONFIG_FILE}: field `{}` is kind = \"enum\" but has no `values`",
                    f.name
                );
            }
        }
        if let Some(s) = &self.project.default_status
            && self.status(s).is_none()
        {
            bail!("{CONFIG_FILE}: project.default_status = `{s}` is not a defined status");
        }
        if let Some(t) = &self.project.default_type
            && self.item_type(t).is_none()
        {
            bail!("{CONFIG_FILE}: project.default_type = `{t}` is not a defined type");
        }
        Ok(())
    }

    pub fn items_dir(&self) -> PathBuf {
        self.root.join(&self.project.dir)
    }

    pub fn status(&self, name: &str) -> Option<&Status> {
        self.statuses.iter().find(|s| s.name == name)
    }

    /// Position in the configured status list — the canonical ordering for
    /// boards, sorting and rendering.
    pub fn status_index(&self, name: &str) -> usize {
        self.statuses
            .iter()
            .position(|s| s.name == name)
            .unwrap_or(usize::MAX)
    }

    pub fn category(&self, status: &str) -> Category {
        self.status(status).map(|s| s.category).unwrap_or_default()
    }

    pub fn item_type(&self, name: &str) -> Option<&ItemType> {
        self.types.iter().find(|t| t.name == name)
    }

    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
    }

    pub fn milestone(&self, name: &str) -> Option<&Milestone> {
        self.milestones.iter().find(|m| m.name == name)
    }

    pub fn view(&self, name: &str) -> Option<&View> {
        self.views.iter().find(|v| v.name == name)
    }

    /// The status new items get when none is specified.
    pub fn initial_status(&self) -> &str {
        self.project
            .default_status
            .as_deref()
            .unwrap_or_else(|| self.statuses[0].name.as_str())
    }

    /// First status in the `done` category — the target of `cairn close`.
    pub fn done_status(&self) -> Option<&Status> {
        self.statuses.iter().find(|s| s.category == Category::Done)
    }

    pub fn format_id(&self, id: u32) -> String {
        format!("{:0width$}", id, width = self.project.id_width)
    }

    /// Bytes available for the slug part of a filename, once the id prefix,
    /// separator and `.md` extension are accounted for.
    pub fn slug_budget(&self) -> usize {
        self.project
            .filename_max
            .saturating_sub(self.project.id_width + 1 + 3)
            .max(8)
    }

    /// The filename an item with this id and title should have.
    pub fn filename_for(&self, id: u32, title: &str) -> String {
        format!(
            "{}-{}.md",
            self.format_id(id),
            crate::item::slug(title, self.slug_budget())
        )
    }

    /// Milestones in the order a reader should meet them.
    ///
    /// Dates order the ones that have them. An undated milestone keeps the
    /// position it was declared in, by taking the date of the next dated
    /// milestone after it — so an `m0-proof` written above a dated `m1-device`
    /// comes first, as its author plainly meant, while a trailing `later` with
    /// nothing dated after it stays at the end.
    ///
    /// Sorting undated milestones to the end unconditionally, as this once did,
    /// overrode an ordering the author had already expressed. Found by real use.
    pub fn milestones_ordered(&self) -> Vec<&Milestone> {
        // Walking backwards lets each undated milestone inherit the date of the
        // nearest dated one that follows it.
        let mut inherited: Vec<Option<&str>> = vec![None; self.milestones.len()];
        let mut next_dated: Option<&str> = None;
        for (i, m) in self.milestones.iter().enumerate().rev() {
            if let Some(d) = m.due.as_deref() {
                next_dated = Some(d);
            }
            inherited[i] = m.due.as_deref().or(next_dated);
        }

        let mut ordered: Vec<(usize, &Milestone)> = self.milestones.iter().enumerate().collect();
        // Undated with nothing dated after it sorts last; declaration order
        // breaks every remaining tie, so the result is stable and explicable.
        ordered.sort_by_key(|(i, _)| (inherited[*i].is_none(), inherited[*i], *i));
        ordered.into_iter().map(|(_, m)| m).collect()
    }
}

/// Field names that are always present on an item and cannot be redefined.
pub const RESERVED_FIELDS: &[&str] = &[
    "id",
    "title",
    "type",
    "status",
    "milestone",
    "labels",
    "assignee",
    "created",
    "updated",
    "depends_on",
    "source",
    "body",
    "category",
];

fn check_unique<'a>(kind: &str, names: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = HashSet::new();
    for n in names {
        if !seen.insert(n) {
            bail!("{CONFIG_FILE}: duplicate [[{kind}]] named `{n}`");
        }
    }
    Ok(())
}
