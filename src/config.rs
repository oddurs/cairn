// cairn — configuration: the schema the whole tool is driven by.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const CONFIG_FILE: &str = "cairn.toml";

/// The on-disk format this build reads and writes.
///
/// Durability is a promise, and this is where it is kept.
///
/// The two halves of a project follow different rules, and saying so was a
/// correction: the sentence here used to cover both and was true of one.
///
/// **An item**, whose keys the specification documents:
///
/// * A patch or minor release may add optional keys, and nothing else.
/// * A reader preserves keys it does not recognise, so a project opened by an
///   older cairn is never silently stripped of data a newer one wrote.
///
/// **The configuration**, which is `deny_unknown_fields`:
///
/// * A new key is a format change. An older cairn does not ignore a key it does
///   not know, it refuses the project, so adding one at the same format number
///   breaks every reader in the field.
/// * That strictness stays. An unknown key in an item came from somewhere and
///   dropping it loses data; an unknown key in `cairn.toml` is a typo, and
///   ignoring it means a project believes it has configured something it has
///   not.
///
/// **Both**:
///
/// * Removing a key, changing its meaning, or making an optional key required
///   needs a new format number, a major release, and a migration.
/// * A project recording a format this build does not know is refused with an
///   explanation, rather than misread.
pub const CURRENT_FORMAT: u32 = 2;

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
    /// Format 1 only. A milestone is an item in format 2, so this exists to be
    /// read by `cairn migrate` and rejected everywhere else.
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
    ///
    /// The older spelling of `id_format`, kept because projects use it.
    /// `id_width = 4` is exactly `id_format = "{n:04}"`.
    #[serde(default = "default_id_width")]
    pub id_width: usize,

    /// Whether `cairn check` reports a closed item whose criteria are unticked.
    ///
    /// Off by default, and the reason is not squeamishness. An unticked box is
    /// not a schema violation the way an undefined status is — it is a judgement
    /// about process — and a project adopting cairn mid-life would get a wall of
    /// warnings about work finished years ago, turn the check off, and then it
    /// would be worth nothing.
    ///
    /// The information is actionable at the moment of closing, which is where
    /// `cairn close` now reports it unconditionally. This is for a project that
    /// wants it enforced afterwards as well.
    #[serde(default)]
    pub require_criteria: bool,

    /// How long a claim may go untouched before it is called stale, in days.
    ///
    /// Absent by default, and inert when absent: a project where a claim means
    /// an afternoon and one where it means a quarter are both real, and neither
    /// is cairn's to guess. Stale means *visible*, never revoked — nothing is
    /// ever released automatically, because taking work away from somebody slow
    /// is worse than leaving it held.
    #[serde(default)]
    pub claim_stale_after: Option<u32>,

    /// The heading acceptance criteria live under, if the project keeps them
    /// somewhere specific.
    ///
    /// Absent, every checkbox in a body counts — because an item that puts its
    /// criteria under a different heading still meant them, and a project that
    /// has not thought about this should not have its criteria silently ignored.
    #[serde(default)]
    pub criteria_section: Option<String>,

    /// How an identifier is written: `{n}` is the number and `{n:04}` pads it.
    ///
    /// `MP-{n}` gives `MP-1002`; `A{n}` gives `A24`. This is a rendering and
    /// nothing more — `id` in the frontmatter is an unsigned integer whatever
    /// this says, so adopting a project key is a display change rather than a
    /// format change, and nothing that refers to an item by number breaks.
    #[serde(default)]
    pub id_format: Option<String>,

    /// The identifier the first item in an empty project takes.
    ///
    /// Allocation is otherwise unchanged: one more than the highest in use. So
    /// lowering this in a project that already has items does nothing, because
    /// the maximum still wins.
    #[serde(default = "default_id_start")]
    pub id_start: u32,
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
    ///
    /// Optional, and kept as an `Option` rather than defaulted away, because
    /// the absence is worth reporting. Every other defaulted key in the file is
    /// presentational — a missing `color` is a colour nobody chose. A missing
    /// `category` is a claim about what a status *means*, and assuming `open`
    /// for a status somebody named `shipped` is the tool getting it exactly
    /// backwards, silently. `cairn check` says so; see `Config::schema_problems`.
    ///
    /// Making it required would be right and costs a format number, so it waits
    /// for one that is being spent anyway.
    #[serde(default, rename = "category")]
    pub declared_category: Option<Category>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    /// Show this status as a column on `cairn board`.
    #[serde(default = "yes")]
    pub board: bool,

    /// What an agent may do about moving an item to this status.
    ///
    /// Separate from writing the `status` field, because the interesting case
    /// is a project that lets an agent start work and not declare it finished.
    #[serde(default)]
    pub agent: Agent,
}

impl Status {
    /// The category this status is treated as having.
    pub fn category(&self) -> Category {
        self.declared_category.unwrap_or_default()
    }

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
    /// A value that names another item rather than describing this one.
    ///
    /// The difference from an enum is that the value has its own existence: a
    /// milestone has a due date, a reason for that date, and a history of the
    /// date moving. An enum value is a string.
    Ref,
}

/// What an agent may do with a field or a status.
///
/// A guard rail rather than a boundary, and the manual says so. The Model
/// Context Protocol server knows who is calling and refuses; a command line
/// cannot, because an agent with a shell can run `cairn set`. Claiming more
/// than that would be worse than offering nothing.
///
/// "Can refuse" was doing a lot of work in an earlier version of this comment.
/// The server applied a caller's fields with `apply` rather than
/// `apply_requested`, so every custom field, every status move and every close
/// went past the check untouched — the command line enforced this and the MCP
/// server did not, which is the exact inverse of what this comment claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    /// Anything a person may do.
    #[default]
    Write,
    /// May be read, not set.
    ReadOnly,
    /// May be asked for, and a person decides. The agent records what it
    /// believes in the item's body, where every other reason already lives.
    Propose,
}

/// How many items a ref field may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Cardinality {
    /// `milestone: v0.1` — an item ships in one release.
    #[default]
    One,
    /// `depends_on: [12, 13]`.
    Many,
}

/// How a ref field's value names its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Addressing {
    /// The identifier. What identity is.
    #[default]
    Id,
    /// A short human handle, unique within the target type.
    ///
    /// This is what keeps `milestone: v0.1` readable in a file. It is not a
    /// second identity: identity is the integer, and a key is a handle, in the
    /// same way a status has a `name` for machines and a `label` for people.
    Key,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldDef {
    pub name: String,
    #[serde(default = "default_kind")]
    pub kind: FieldKind,

    /// For `kind = "ref"`: the type of item this names, or `*` for any.
    ///
    /// A field naming a *specific* type makes that type a container: its items
    /// are the thing work belongs to rather than work itself.
    #[serde(default)]
    pub target: Option<String>,

    #[serde(default)]
    pub cardinality: Cardinality,

    #[serde(default)]
    pub by: Addressing,

    /// Refuse a value that would close a cycle.
    #[serde(default)]
    pub acyclic: bool,

    /// Contribute to the progress of whatever is named.
    #[serde(default)]
    pub rollup: bool,

    /// What to call the question asked backwards: `contains`, `blocks`.
    #[serde(default)]
    pub inverse: Option<String>,

    /// What an agent may do with this field.
    #[serde(default)]
    pub agent: Agent,
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

impl Hooks {
    /// How many events have a hook attached. A bug report says this because a
    /// configured hook is the most common reason cairn appears to do something
    /// it does not do.
    pub fn count(&self) -> usize {
        [
            &self.after_create,
            &self.after_change,
            &self.after_remove,
            &self.after_render,
        ]
        .iter()
        .filter(|h| h.is_some())
        .count()
    }
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
/// How a project writes its identifiers.
///
/// `id` is an unsigned integer and stays one. This is only how that integer is
/// shown and read back, which is why adopting `MP-{n}` is a display change
/// rather than a format change: nothing stored moves, and every reference
/// written as a bare number keeps working.
///
/// Deliberately one template rather than three settings for prefix, separator
/// and padding. Three would express the same thing less clearly and would allow
/// combinations nobody wants; a template shows you what it produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdFormat {
    prefix: String,
    pad: usize,
    suffix: String,
}

impl IdFormat {
    /// The default, and the older `id_width` spelling: `0001`.
    pub fn padded(width: usize) -> IdFormat {
        IdFormat {
            prefix: String::new(),
            pad: width,
            suffix: String::new(),
        }
    }

    /// `MP-{n}`, `A{n}`, `{n:04}`.
    pub fn compile(template: &str) -> Result<IdFormat> {
        let Some(open) = template.find('{') else {
            bail!(
                "id_format `{template}` has no `{{n}}`: the template says where \
                 the number goes, so it has to contain one"
            );
        };
        let Some(close) = template[open..].find('}').map(|i| open + i) else {
            bail!("id_format `{template}`: `{{` is never closed");
        };
        let prefix = template[..open].to_string();
        let suffix = template[close + 1..].to_string();

        if suffix.contains('{') {
            bail!(
                "id_format `{template}` has more than one placeholder: an item \
                 has one identifier"
            );
        }

        let inner = &template[open + 1..close];
        let pad = match inner {
            "n" => 1,
            _ => match inner.strip_prefix("n:0") {
                Some(digits) => digits.parse::<usize>().map_err(|_| {
                    anyhow::anyhow!(
                        "id_format `{template}`: `{{{inner}}}` should be `{{n}}` \
                         or `{{n:0W}}` for a width, as in `{{n:04}}`"
                    )
                })?,
                None => bail!(
                    "id_format `{template}`: `{{{inner}}}` should be `{{n}}` or \
                     `{{n:0W}}` for a width, as in `{{n:04}}`"
                ),
            },
        };

        // A prefix that is itself digits would make the rendered form
        // ambiguous with a bare number, and `MP-12` and `12` must not be able
        // to mean different items.
        if prefix.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            bail!(
                "id_format `{template}`: a prefix cannot start with a digit, or \
                 the rendered identifier could not be told from a plain number"
            );
        }
        if pad > 18 {
            bail!("id_format `{template}`: a width of {pad} is not a width");
        }
        Ok(IdFormat {
            prefix,
            pad,
            suffix,
        })
    }

    pub fn render(&self, id: u32) -> String {
        format!(
            "{}{:0width$}{}",
            self.prefix,
            id,
            self.suffix,
            width = self.pad
        )
    }

    /// A typical rendered width, for budgeting filename length.
    pub fn width(&self) -> usize {
        self.prefix.chars().count() + self.pad.max(4) + self.suffix.chars().count()
    }

    /// Accept the rendered form, or the bare number, with an optional `#`.
    ///
    /// Case-insensitively for the prefix and suffix: somebody typing
    /// `cairn show mp-1002` meant the same item, and refusing them is pedantry.
    pub fn read(&self, s: &str) -> Result<u32> {
        let raw = s.trim();
        let t = raw.trim_start_matches('#').trim();

        // A missing prefix is not an error: the bare number is always
        // acceptable, and every reference written before the project adopted a
        // key is one.
        let mut rest = t;
        if let Some(r) = strip_prefix_ci(rest, &self.prefix) {
            rest = r;
        }
        if !self.suffix.is_empty()
            && let Some(r) = strip_suffix_ci(rest, &self.suffix)
        {
            rest = r;
        }

        match rest.parse::<u32>() {
            Ok(n) => Ok(n),
            Err(_) => {
                let example = self.render(12);
                bail!("`{raw}` is not a valid item id (expected {example}, or 12)")
            }
        }
    }

    /// The identifier a filename begins with, if it is one this format could
    /// have produced.
    pub fn id_in_filename(&self, name: &str) -> Option<u32> {
        let stem = name.strip_suffix(".md").unwrap_or(name);
        let rest = if self.prefix.is_empty() {
            stem
        } else {
            strip_prefix_ci(stem, &self.prefix)?
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return None;
        }
        digits.parse().ok()
    }
}

/// Both of these ask for a slice at a byte offset, and both use `get` to do it.
///
/// A guard in bytes followed by a slice in bytes is correct arithmetic and the
/// wrong question: the offset can land inside a character, and slicing there
/// panics. `MP` is two bytes, `é` is two bytes, and `cairn show 'aé'` was enough
/// to bring the process down. `get` returns `None` for an offset that is not a
/// boundary, which is the honest answer — a string whose second byte is halfway
/// through a character does not start with `MP`.
/// Say more about an unknown configuration key than serde can.
///
/// serde lists the keys it expected, which is genuinely useful and stops one
/// step short of the question being asked. Two very different things arrive
/// looking identical: a typo, and a file written by a newer cairn. The first
/// wants the nearest key; the second wants to know that `cairn.toml` refuses
/// keys rather than ignoring them, so this is not a bug to hunt.
fn advise_on_unknown_key(message: &str) -> anyhow::Error {
    let Some(advice) = unknown_key_advice(message) else {
        return anyhow::anyhow!("{message}");
    };
    anyhow::anyhow!("{message}\n{advice}")
}

fn unknown_key_advice(message: &str) -> Option<String> {
    let (_, rest) = message.split_once("unknown field `")?;
    let (found, rest) = rest.split_once('`')?;

    let mut advice = String::new();
    if let Some((_, list)) = rest.split_once("expected one of ") {
        let candidates: Vec<&str> = list
            .split(',')
            .filter_map(|s| s.trim().trim_end_matches('\n').split('`').nth(1))
            .collect();
        // Within two edits: far enough to catch a transposition or a dropped
        // letter, near enough that the suggestion is not noise.
        if let Some(near) = candidates
            .iter()
            .map(|c| (edits(found, c), *c))
            .filter(|(d, _)| *d <= 2)
            .min_by_key(|(d, _)| *d)
            .map(|(_, c)| c)
        {
            advice.push_str(&format!("did you mean `{near}`?\n"));
        }
    }
    advice.push_str(&format!(
        "this is cairn {}, which reads format {CURRENT_FORMAT}. A key it does not know is\n\
         refused rather than ignored, so a key from a newer cairn arrives looking like a typo.",
        env!("CARGO_PKG_VERSION"),
    ));
    Some(advice)
}

/// Levenshtein distance, iterative and small. Written out rather than pulled in:
/// one suggestion in one error message is not worth a dependency.
fn edits(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        s.get(prefix.len()..)
    } else {
        None
    }
}

fn strip_suffix_ci<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let cut = s.len().checked_sub(suffix.len())?;
    let tail = s.get(cut..)?;
    if tail.eq_ignore_ascii_case(suffix) {
        s.get(..cut)
    } else {
        None
    }
}

fn default_id_start() -> u32 {
    1
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
            require_criteria: false,
            claim_stale_after: None,
            criteria_section: None,
            id_format: None,
            id_start: default_id_start(),
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
        Config::check_format(&text, path, true)?;

        let mut cfg: Config = toml::from_str(&text)
            .map_err(|e| advise_on_unknown_key(&e.to_string()))
            .with_context(|| format!("parsing {}", path.display()))?;
        cfg.root = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        cfg.validate()?;
        cfg.notice_if_behind();
        Ok(cfg)
    }

    /// Say once, on standard error, that the project is behind.
    ///
    /// Standard error because standard output is somebody's `--json`, and a
    /// notice that breaks a pipeline is worse than no notice. Once because
    /// `Config::load` runs more than once in some commands and nobody needs to
    /// be told twice.
    fn notice_if_behind(&self) {
        static SAID: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        if self.format() >= CURRENT_FORMAT || SAID.set(()).is_err() {
            return;
        }
        eprintln!(
            "{} this project is format {}; reading works, writing needs `cairn migrate`",
            crate::style::dim("note:"),
            self.format()
        );
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
    /// Load a project that may be behind the current format.
    ///
    /// Only `cairn migrate` uses this. Everything else refuses an older project
    /// rather than reading it on a best-effort basis, which §8 of the
    /// specification requires and which is the whole reason the version exists.
    pub fn load_for_migration(path: &Path) -> Result<Config> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Config::check_format(&text, path, false)?;
        let mut cfg: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        cfg.root = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Ok(cfg)
    }

    fn check_format(text: &str, path: &Path, strict: bool) -> Result<()> {
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
        // An older project is *not* refused. §8 requires refusing a version a
        // reader does not understand, which is about a version from the future,
        // where best-effort reading means misreading data nobody can predict.
        // This cairn understands format 1 exactly, and refusing to show
        // somebody their own backlog over a command they have not run yet is
        // the tool being difficult for its own sake.
        //
        // Writing is where it stops: see `Lock::acquire`.
        let _ = strict;
        Ok(())
    }

    /// The format this project records, defaulting to 1.
    pub fn format(&self) -> u32 {
        self.format.unwrap_or(1)
    }

    fn validate(&self) -> Result<()> {
        // First, because a project whose identifiers cannot be rendered has no
        // way to report anything else it might be wrong about.
        if let Some(template) = &self.project.id_format {
            IdFormat::compile(template).map_err(|e| anyhow::anyhow!("{CONFIG_FILE}: {e}"))?;
        }
        if self.statuses.is_empty() {
            bail!("{CONFIG_FILE}: at least one [[status]] must be defined");
        }
        check_unique("status", self.statuses.iter().map(|s| s.name.as_str()))?;
        check_unique("type", self.types.iter().map(|t| t.name.as_str()))?;
        check_unique("field", self.fields.iter().map(|f| f.name.as_str()))?;
        // Two different situations wore one sentence, and the sentence was
        // right for only one of them. At format 1 there is a migration to run.
        // At the current format there is not — `cairn migrate` answers "nothing
        // to migrate" — so telling somebody to run it left them between two
        // commands that contradicted each other with nothing to do about it.
        if !self.milestones.is_empty() && self.format() >= CURRENT_FORMAT {
            let names: Vec<&str> = self.milestones.iter().map(|m| m.name.as_str()).collect();
            let has_type = self.item_type(crate::refs::MILESTONE_TYPE).is_some();
            let has_field = self.field(crate::refs::MILESTONE_FIELD).is_some();
            bail!(
                "{CONFIG_FILE}: [[milestone]] blocks are format 1 and are no longer read.\n\
                 a milestone is an item now, so delete the block{} ({}) and write {} instead:\n\
                 \n    cairn new \"{}\" -t milestone\n\n\
                 this project {} the `[[type]]` and `[[field]]` that replace them",
                if names.len() == 1 { "" } else { "s" },
                names.join(", "),
                if names.len() == 1 {
                    "the item"
                } else {
                    "items"
                },
                names.first().copied().unwrap_or("v0.1"),
                match (has_type, has_field) {
                    (true, true) => "already has",
                    (false, false) => "is missing both",
                    (true, false) => "has the type but not the field:",
                    (false, true) => "has the field but not the type:",
                }
            );
        }
        check_unique("view", self.views.iter().map(|v| v.name.as_str()))?;

        for f in &self.fields {
            // A reserved key may be *declared* when it is one of the two that
            // are references — `milestone` and `depends_on`. They stay typed on
            // the item, because the specification documents them, and the
            // declaration is what lets the general mechanism describe and
            // validate them instead of a special case doing it.
            let redeclarable =
                f.kind == FieldKind::Ref && matches!(f.name.as_str(), "milestone" | "depends_on");
            if RESERVED_FIELDS.contains(&f.name.as_str()) && !redeclarable {
                bail!(
                    "{CONFIG_FILE}: [[field]] name `{}` is reserved (built-in field)",
                    f.name
                );
            }
            if f.kind == FieldKind::Ref {
                let target = f.target.as_deref().unwrap_or("*");
                if target != "*" && self.item_type(target).is_none() {
                    bail!(
                        "{CONFIG_FILE}: field `{}` targets `{target}`, which is not a \
                         declared [[type]]",
                        f.name
                    );
                }
            } else if f.target.is_some() {
                bail!(
                    "{CONFIG_FILE}: field `{}` has a `target` but is not kind = \"ref\"",
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
        self.status(status)
            .map(|s| s.category())
            .unwrap_or_default()
    }

    pub fn item_type(&self, name: &str) -> Option<&ItemType> {
        self.types.iter().find(|t| t.name == name)
    }

    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.fields.iter().find(|f| f.name == name)
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
        self.statuses
            .iter()
            .find(|s| s.category() == Category::Done)
    }

    pub fn format_id(&self, id: u32) -> String {
        self.id_format().render(id)
    }

    /// The project's identifier rendering.
    ///
    /// Parsed on every call rather than cached: it is a handful of characters,
    /// this is not on any hot path, and a cache on `Config` would have to be
    /// kept correct through `Deserialize`, which is exactly the kind of
    /// invariant that rots.
    pub fn id_format(&self) -> IdFormat {
        match &self.project.id_format {
            Some(template) => IdFormat::compile(template)
                // Validated at load, so this cannot be reached from a project
                // cairn has opened.
                .unwrap_or_else(|_| IdFormat::padded(self.project.id_width)),
            None => IdFormat::padded(self.project.id_width),
        }
    }

    /// Read an identifier the user typed: the rendered form, or the bare
    /// number, or either with a leading `#`.
    ///
    /// Both are accepted because requiring a prefix somebody already knows is
    /// friction for nothing, and because every reference written before the
    /// project adopted a key is a bare number.
    pub fn parse_id(&self, s: &str) -> Result<u32> {
        self.id_format().read(s)
    }

    /// Bytes available for the slug part of a filename, once the id prefix,
    /// separator and `.md` extension are accounted for.
    pub fn slug_budget(&self) -> usize {
        self.project
            .filename_max
            // The rendered identifier, the separating dash, and ".md".
            .saturating_sub(self.id_format().width() + 1 + 3)
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
}

/// Field names that are always present on an item and cannot be redefined.
pub const RESERVED_FIELDS: &[&str] = &[
    "id",
    "key",
    "title",
    "type",
    "status",
    "milestone",
    "labels",
    "assignee",
    "claimed",
    "owner",
    "created_by",
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
