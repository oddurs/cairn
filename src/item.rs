// cairn — the item: one Markdown file with YAML frontmatter.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer};
use serde_yaml_ng::{Mapping, Value};
use std::path::{Path, PathBuf};

/// How a file separates its lines.
///
/// Item files are edited by people on every platform and by git clients that
/// rewrite line endings in transit. cairn reads either, keeps track of which it
/// found, and writes the same back — so a checkout with `core.autocrlf` set
/// does not turn every `cairn set` into a whole-file diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Eol {
    #[default]
    Lf,
    Crlf,
}

impl Eol {
    /// Whichever ending the file mostly uses. A file with no line breaks at all
    /// gets LF, which is also what new items are written with.
    pub fn detect(text: &str) -> Eol {
        let crlf = text.matches("\r\n").count();
        let lf = text.matches('\n').count() - crlf;
        if crlf > lf { Eol::Crlf } else { Eol::Lf }
    }

    /// Rewrite LF-separated text in this ending.
    pub fn apply(self, text: &str) -> String {
        match self {
            Eol::Lf => text.to_string(),
            Eol::Crlf => text.replace('\n', "\r\n"),
        }
    }
}

/// A resolved field value, flattened to the shapes the CLI actually reasons
/// about: a scalar, a list, or nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Text(String),
    List(Vec<String>),
    Missing,
}

impl Field {
    pub fn is_missing(&self) -> bool {
        match self {
            Field::Missing => true,
            Field::Text(s) => s.is_empty(),
            Field::List(v) => v.is_empty(),
        }
    }

    /// Single-line rendering for tables and templates.
    pub fn display(&self) -> String {
        match self {
            Field::Text(s) => s.clone(),
            Field::List(v) => v.join(", "),
            Field::Missing => String::new(),
        }
    }

    /// Every value this field could match against, for filter comparisons.
    pub fn values(&self) -> Vec<&str> {
        match self {
            Field::Text(s) => vec![s.as_str()],
            Field::List(v) => v.iter().map(String::as_str).collect(),
            Field::Missing => vec![],
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub id: Option<u32>,
    #[serde(default)]
    pub title: Option<String>,
    /// A short human handle, unique among items of this type.
    ///
    /// Not an identity — identity is `id`. This is what a ref field addressed
    /// `by = "key"` names, and it is why `milestone: v0.1` stays readable
    /// instead of becoming `milestone: 42`.
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub milestone: Option<String>,
    /// Who is working on it. `claim` sets this.
    #[serde(default)]
    pub assignee: Option<String>,
    /// When the current claim was taken.
    ///
    /// Its own key rather than reading `updated`, which means something else
    /// and which every other edit moves. A claim is a lock on *intent*, and it
    /// was the only lock here with nothing recording when it was taken — so a
    /// session that died left an item claimed for ever, invisible to `next`
    /// because it was held and invisible to a person because nothing listed it.
    #[serde(default)]
    pub claimed: Option<String>,
    /// Who is answerable for it, which is a different question.
    ///
    /// With people the two are usually the same, which is why one field served.
    /// With an agent working and a person owning they differ, and claiming used
    /// to overwrite the record of who cared.
    #[serde(default)]
    pub owner: Option<String>,
    /// What created it, as cairn was told: a person's name, or an agent's.
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default, deserialize_with = "de_string_list")]
    pub labels: Vec<String>,
    #[serde(default, deserialize_with = "de_id_list")]
    pub depends_on: Vec<u32>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    /// Where this item came from, if it was imported: `github:owner/repo#12`.
    /// Import matches on it, so re-importing updates rather than duplicates.
    #[serde(default)]
    pub source: Option<String>,
    /// Anything else in the frontmatter: the user's custom fields.
    #[serde(flatten)]
    pub extra: Mapping,
}

/// The state of an item's acceptance criteria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Criteria {
    pub done: usize,
    pub total: usize,
}

impl Criteria {
    pub fn any(&self) -> bool {
        self.total > 0
    }

    /// Whether every criterion is ticked. An item that states none is
    /// vacuously complete, which matters: most items have no criteria and must
    /// not be reported as unfinished.
    pub fn complete(&self) -> bool {
        self.done == self.total
    }

    pub fn display(&self) -> String {
        format!("{}/{}", self.done, self.total)
    }
}

/// One acceptance criterion, and where it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    /// Zero-based index into the body's lines.
    pub line: usize,
    pub ticked: bool,
    /// What the criterion says, without the list marker or the box.
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: u32,
    pub meta: Meta,
    pub body: String,
    pub path: PathBuf,
    /// The raw frontmatter, kept so diagnostics can report the line a field is
    /// defined on. Empty for items built in memory rather than read from disk.
    pub front: String,
    /// The line ending this file used, reproduced when it is written back.
    pub eol: Eol,
}

impl Item {
    pub fn key(&self) -> Option<&str> {
        self.meta.key.as_deref().filter(|k| !k.trim().is_empty())
    }

    pub fn title(&self) -> &str {
        self.meta.title.as_deref().unwrap_or("(untitled)")
    }

    pub fn status(&self) -> &str {
        self.meta.status.as_deref().unwrap_or("")
    }

    pub fn kind(&self) -> Option<&str> {
        self.meta.kind.as_deref()
    }

    pub fn milestone(&self) -> Option<&str> {
        self.meta.milestone.as_deref()
    }

    /// Look up any field by name — built-in or custom. `category` is resolved by
    /// the caller, which has the config.
    /// Names `get` answers that are not the field's own name.
    ///
    /// Beside `get` for the same reason `DERIVED_KEYS` sits beside `resolve`:
    /// the alias and the list of legitimate keys have to move together.
    /// `cairn check` learned this the hard way, by calling a working saved view
    /// in a real project a typo the first time it was run outside the tests.
    pub const ALIASES: &'static [&'static str] = &["kind", "label"];

    pub fn get(&self, key: &str) -> Field {
        match key {
            "id" => Field::Text(self.id.to_string()),
            "title" => opt(self.meta.title.as_deref()),
            "key" => opt(self.key()),
            "type" | "kind" => opt(self.meta.kind.as_deref()),
            "status" => opt(self.meta.status.as_deref()),
            "milestone" => opt(self.meta.milestone.as_deref()),
            "assignee" => opt(self.meta.assignee.as_deref()),
            "claimed" => opt(self.meta.claimed.as_deref()),
            "owner" => opt(self.meta.owner.as_deref()),
            "created_by" => opt(self.meta.created_by.as_deref()),
            "created" => opt(self.meta.created.as_deref()),
            "updated" => opt(self.meta.updated.as_deref()),
            "source" => opt(self.meta.source.as_deref()),
            "labels" | "label" => Field::List(self.meta.labels.clone()),
            "depends_on" => Field::List(self.meta.depends_on.iter().map(u32::to_string).collect()),
            "body" => Field::Text(self.body.clone()),
            other => match self.meta.extra.get(Value::String(other.to_string())) {
                Some(v) => value_to_field(v),
                None => Field::Missing,
            },
        }
    }

    /// Set a custom (non-built-in) field. Built-ins are handled by callers so
    /// they can validate against the schema first.
    /// Set a field whose values are identifiers, storing them as numbers.
    ///
    /// An id is a number, and writing it as `'1'` would both look wrong beside
    /// `depends_on: [1, 2]` and rewrite a hand-written `part_of: [1, 4]` into
    /// quoted strings on the next save — turning an unchanged item into a diff.
    pub fn set_extra_ids(&mut self, key: &str, ids: &[u32]) {
        let k = Value::String(key.to_string());
        if ids.is_empty() {
            self.meta.extra.remove(&k);
            return;
        }
        let values: Vec<Value> = ids.iter().map(|n| Value::Number((*n).into())).collect();
        self.meta.extra.insert(k, Value::Sequence(values));
    }

    /// The same, for a field that holds exactly one identifier.
    pub fn set_extra_id(&mut self, key: &str, id: Option<u32>) {
        let k = Value::String(key.to_string());
        match id {
            None => {
                self.meta.extra.remove(&k);
            }
            Some(n) => {
                self.meta.extra.insert(k, Value::Number(n.into()));
            }
        }
    }

    pub fn set_extra(&mut self, key: &str, value: Option<Field>) {
        let k = Value::String(key.to_string());
        match value {
            None => {
                self.meta.extra.remove(&k);
            }
            Some(Field::Missing) => {
                self.meta.extra.remove(&k);
            }
            Some(Field::Text(s)) => {
                self.meta.extra.insert(k, Value::String(s));
            }
            Some(Field::List(v)) => {
                self.meta.extra.insert(
                    k,
                    Value::Sequence(v.into_iter().map(Value::String).collect()),
                );
            }
        }
    }

    /// The first paragraph of the body — used for one-line summaries.
    pub fn summary(&self) -> String {
        self.body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("<!--"))
            .unwrap_or("")
            .to_string()
    }

    /// How many acceptance criteria this item states, and how many are ticked.
    ///
    /// Every item cairn's own templates produce carries `- [ ]` boxes under a
    /// heading, and until this existed nothing read them: an item could close
    /// with every box empty and `check --strict` was satisfied. That is the gap
    /// between claiming items carry the thinking — including how you will know
    /// when it is done — and being able to verify it.
    ///
    /// `section`, when given, restricts the count to the lines under a heading
    /// of that name. Without it every box in the body counts, because an item
    /// that puts its criteria somewhere else still meant them.
    pub fn criteria(&self, section: Option<&str>) -> Criteria {
        let list = self.criteria_list(section);
        Criteria {
            done: list.iter().filter(|c| c.ticked).count(),
            total: list.len(),
        }
    }

    /// The same criteria, each with the line it is written on.
    ///
    /// One parser, because there is no version of this where the number
    /// `cairn tick 12 3` takes and the count `cairn show` prints are allowed to
    /// disagree: they would drift the first time somebody decided an indented
    /// box was or was not a criterion.
    pub fn criteria_list(&self, section: Option<&str>) -> Vec<Criterion> {
        let mut out = Vec::new();
        // `None` before the first heading means "counting", so a body with no
        // headings at all still works.
        let mut counting = section.is_none();

        for (n, line) in self.body.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(heading) = trimmed.strip_prefix('#') {
                if let Some(want) = section {
                    let name = heading.trim_start_matches('#').trim();
                    counting = name.eq_ignore_ascii_case(want);
                }
                continue;
            }
            if !counting {
                continue;
            }
            // A list marker, then a box. Indentation is allowed so that nested
            // criteria count; anything else on the line is the criterion.
            let Some(rest) = trimmed
                .strip_prefix("- ")
                .or_else(|| trimmed.strip_prefix("* "))
                .or_else(|| trimmed.strip_prefix("+ "))
            else {
                continue;
            };
            let rest = rest.trim_start();
            let (ticked, after) = if let Some(after) = rest.strip_prefix("[ ]") {
                (false, after)
            } else if let Some(after) = rest
                .strip_prefix("[x]")
                .or_else(|| rest.strip_prefix("[X]"))
            {
                (true, after)
            } else {
                continue;
            };
            // A box with nothing after it is a placeholder, not a criterion.
            // The type template cairn ships ends with a bare `- [ ]` prompting
            // the author to write one, so counting it would make every item
            // ever created report one unticked criterion forever — which is
            // exactly the noise that gets a feature switched off.
            if !after.starts_with(char::is_whitespace) || after.trim().is_empty() {
                continue;
            }
            out.push(Criterion {
                line: n,
                ticked,
                text: after.trim().to_string(),
            });
        }
        out
    }

    /// Set the state of the criteria at these line numbers, and say whether the
    /// body actually changed.
    ///
    /// Line numbers rather than ordinals because the caller has already made
    /// the ordinal mean something — which criteria the numbers on the command
    /// line picked out — and re-deriving it here would be a second chance to
    /// get it wrong. Rewriting only the box keeps everything else on the line,
    /// including whatever indentation and trailing notes the author wrote.
    pub fn set_criteria(&mut self, lines: &[usize], ticked: bool) -> bool {
        let want = if ticked { "[x]" } else { "[ ]" };
        let mut changed = false;
        let mut out: Vec<String> = Vec::new();
        for (n, line) in self.body.lines().enumerate() {
            if !lines.contains(&n) {
                out.push(line.to_string());
                continue;
            }
            // The box is the first `[` after the list marker, and `criteria_list`
            // has already established that this line has one.
            match line.find(['[']) {
                Some(at) if line.len() >= at + 3 => {
                    let mut next = String::with_capacity(line.len());
                    next.push_str(&line[..at]);
                    next.push_str(want);
                    next.push_str(&line[at + 3..]);
                    changed |= next != line;
                    out.push(next);
                }
                _ => out.push(line.to_string()),
            }
        }
        if changed {
            let trailing = self.body.ends_with('\n');
            self.body = out.join("\n");
            if trailing {
                self.body.push('\n');
            }
        }
        changed
    }

    pub fn parse(path: &Path, text: &str) -> Result<Item> {
        Item::parse_with(path, text, None)
    }

    /// Parse, optionally recovering a missing `id` through the project's
    /// identifier rendering as well as the leading-digits rule.
    ///
    /// A project whose identifiers read `MP-1002` has files called
    /// `MP-1002-slug.md`, which begin with no digits at all — so a hand-written
    /// file there would be unreadable, and the specification's digits fallback
    /// covers only renderings that start with the number. §4.2 permits a reader
    /// to apply the project's rendering instead, and this is that.
    pub fn parse_with(
        path: &Path,
        text: &str,
        format: Option<&crate::config::IdFormat>,
    ) -> Result<Item> {
        let eol = Eol::detect(text);
        // Everything is handled as LF internally; the original ending is
        // reapplied on the way out.
        let normalised = text.replace("\r\n", "\n");
        let text = normalised.as_str();
        let (front, body) = split_frontmatter(text).ok_or_else(|| {
            anyhow::anyhow!(
                "{}: missing YAML frontmatter (a file must start with a `---` line)",
                at(path, Some(1))
            )
        })?;
        let meta: Meta = serde_yaml_ng::from_str(&front).map_err(|e| {
            // Frontmatter starts on line 2; serde reports lines within it.
            let line = e.location().map(|l| l.line() + 1);
            anyhow::anyhow!("{}: invalid frontmatter: {e}", at(path, line))
        })?;
        let id = meta
            .id
            .or_else(|| id_from_filename(path))
            .or_else(|| {
                let name = path.file_name()?.to_str()?;
                format?.id_in_filename(name)
            })
            .ok_or_else(|| {
                let expected = match format {
                    Some(f) => format!("does not match `{}`", f.render(12)),
                    None => "does not start with a number".to_string(),
                };
                anyhow::anyhow!(
                    "{}: no `id:` in frontmatter and the filename {expected}",
                    at(path, Some(2))
                )
            })?;
        Ok(Item {
            id,
            meta,
            body: body.trim_start_matches('\n').to_string(),
            path: path.to_path_buf(),
            front,
            eol,
        })
    }

    /// The file line a frontmatter key is defined on, for diagnostics.
    pub fn line_of(&self, key: &str) -> Option<usize> {
        if self.front.is_empty() {
            return None;
        }
        let prefix = format!("{key}:");
        self.front
            .lines()
            .position(|l| l.starts_with(&prefix))
            // +2: line 1 is the opening `---`, and `position` is 0-based.
            .map(|i| i + 2)
    }

    pub fn load_with(path: &Path, format: Option<&crate::config::IdFormat>) -> Result<Item> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Item::parse_with(path, &text, format)
    }

    pub fn load(path: &Path) -> Result<Item> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Item::parse(path, &text)
    }

    /// Render back to Markdown. Key order is fixed so diffs stay readable.
    pub fn to_markdown(&self) -> Result<String> {
        let mut m = Mapping::new();
        let mut put = |k: &str, v: Value| {
            m.insert(Value::String(k.to_string()), v);
        };
        put("id", Value::Number(self.id.into()));
        if let Some(v) = &self.meta.key {
            put("key", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.title {
            put("title", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.kind {
            put("type", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.status {
            put("status", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.milestone {
            put("milestone", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.assignee {
            put("assignee", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.claimed {
            put("claimed", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.owner {
            put("owner", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.created_by {
            put("created_by", Value::String(v.clone()));
        }
        if !self.meta.labels.is_empty() {
            put(
                "labels",
                Value::Sequence(
                    self.meta
                        .labels
                        .iter()
                        .cloned()
                        .map(Value::String)
                        .collect(),
                ),
            );
        }
        if !self.meta.depends_on.is_empty() {
            put(
                "depends_on",
                Value::Sequence(
                    self.meta
                        .depends_on
                        .iter()
                        .map(|i| Value::Number((*i).into()))
                        .collect(),
                ),
            );
        }
        if let Some(v) = &self.meta.created {
            put("created", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.updated {
            put("updated", Value::String(v.clone()));
        }
        if let Some(v) = &self.meta.source {
            put("source", Value::String(v.clone()));
        }
        for (k, v) in &self.meta.extra {
            m.insert(k.clone(), v.clone());
        }

        let yaml =
            serde_yaml_ng::to_string(&Value::Mapping(m)).context("serialising frontmatter")?;
        // Bodies arrive from seven places — an argument, stdin, an editor, an
        // import document, three MCP tools — and any of them can carry CRLF.
        // Normalising here rather than at each entry means no future one can
        // reintroduce the fault: a body that reached the struct with CRLF in it
        // would otherwise be given another CR by `eol.apply`, writing `\r\r\n`,
        // and enough such lines flip what `Eol::detect` reads back.
        let body = self.body.replace("\r\n", "\n");
        let body = body.trim_end();
        // Rendered with LF throughout, then given back whatever ending the file
        // arrived with.
        Ok(self.eol.apply(&format!("---\n{}---\n\n{}\n", yaml, body)))
    }

    pub fn save(&self) -> Result<()> {
        let text = self.to_markdown()?;
        crate::store::write_atomic(&self.path, text.as_bytes())
    }

    /// Set the body from outside, normalising line endings on the way in.
    ///
    /// `Item::parse` guarantees a body read from disk holds no CRLF; this keeps
    /// that true for bodies that never came from disk.
    /// Append a note under a heading, leaving what is already there alone.
    ///
    /// The rule every caller wants and two of them had implemented separately:
    /// one blank line between what was there and what is being added, whatever
    /// the body ended with. A note can never erase, which is the whole reason
    /// there is a `note` command distinct from setting the body.
    pub fn append_note(&mut self, heading: &str, text: &str) {
        let addition = format!("## {heading}\n\n{text}");
        let body = self.body.trim_end();
        let combined = if body.is_empty() {
            addition
        } else {
            format!("{body}\n\n{addition}")
        };
        self.set_body(&combined);
    }

    /// The text under the most recent heading beginning with `prefix`.
    ///
    /// Used to find the last reason somebody handed an item back, so the next
    /// taker meets it rather than discovering the same dead end.
    pub fn last_note(&self, prefix: &str) -> Option<String> {
        let mut found: Option<String> = None;
        let mut collecting = false;
        let mut text = String::new();
        for line in self.body.lines() {
            if let Some(heading) = line.trim().strip_prefix("## ") {
                if collecting {
                    found = Some(text.trim().to_string());
                }
                collecting = heading.starts_with(prefix);
                text.clear();
                continue;
            }
            if collecting {
                text.push_str(line);
                text.push('\n');
            }
        }
        if collecting {
            found = Some(text.trim().to_string());
        }
        found.filter(|f| !f.is_empty())
    }

    pub fn set_body(&mut self, text: &str) {
        self.body = text.replace("\r\n", "\n");
    }

    pub fn touch(&mut self, today: &str) {
        self.meta.updated = Some(today.to_string());
    }
}

/// Format a source location the way the GNU Coding Standards prescribe:
/// `file:line`, or bare `file` when there is no meaningful line.
pub fn at(path: &Path, line: Option<usize>) -> String {
    match line {
        Some(l) => format!("{}:{l}", path.display()),
        None => path.display().to_string(),
    }
}

fn opt(s: Option<&str>) -> Field {
    match s {
        Some(v) if !v.is_empty() => Field::Text(v.to_string()),
        _ => Field::Missing,
    }
}

pub fn value_to_field(v: &Value) -> Field {
    match v {
        Value::Null => Field::Missing,
        Value::String(s) => Field::Text(s.clone()),
        Value::Bool(b) => Field::Text(b.to_string()),
        Value::Number(n) => Field::Text(n.to_string()),
        Value::Sequence(seq) => Field::List(seq.iter().filter_map(scalar_string).collect()),
        _ => Field::Text(String::new()),
    }
}

fn scalar_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Split `---\n…\n---\n` frontmatter from the body.
pub fn split_frontmatter(text: &str) -> Option<(String, String)> {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    let after_open = t.strip_prefix("---")?;
    let nl = after_open.find('\n')?;
    if !after_open[..nl].trim().is_empty() {
        return None;
    }
    let rest = &after_open[nl + 1..];
    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed == "..." {
            return Some((
                rest[..offset].to_string(),
                rest[offset + line.len()..].to_string(),
            ));
        }
        offset += line.len();
    }
    None
}

fn id_from_filename(path: &Path) -> Option<u32> {
    let stem = path.file_stem()?.to_str()?;
    let digits: String = stem.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// The key an item of a grouping type gets when nobody supplies one.
///
/// Deliberately not `slug`. A filename may lose whatever punctuation it likes,
/// because nobody types it; a key is exactly what somebody types — `cairn set
/// 12 milestone=v1.0` — so `v1.0` has to survive the trip from the title. The
/// transformation is the smallest one that still yields a single unquoted
/// word: lowercase, runs of anything else become one dash, and the characters
/// version numbers are made of are kept.
///
/// `Usable in anger` -> `usable-in-anger`, `v1.0` -> `v1.0`, `Q1 2027` ->
/// `q1-2027`.
pub fn key_from_title(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            // Filtered *after* lowercasing, because lowercasing can introduce a
            // character that is not alphanumeric: `İ` becomes `i` followed by a
            // combining dot, and letting the mark through puts an invisible
            // character in the middle of something somebody has to type.
            let lowered: String = c.to_lowercase().filter(|c| c.is_alphanumeric()).collect();
            if lowered.is_empty() {
                continue;
            }
            out.push_str(&lowered);
            dash = false;
        } else if matches!(c, '.' | '_' | '+') && !out.is_empty() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches(|c| c == '-' || c == '.').to_string()
}

/// `Add OAuth login!` -> `add-oauth-login`
///
/// `max_bytes` is a filesystem constraint, not a style choice: the caller
/// derives it from the longest filename the target system accepts, so long
/// titles are only ever shortened as much as the filesystem demands.
pub fn slug(title: &str, max_bytes: usize) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            // Filtered after lowercasing rather than before: `İ` is alphanumeric
            // and lowercases to `i` plus a combining dot, which is not — so
            // testing the original character alone let a combining mark into a
            // filename. Found by the property test below, on `title = "İ"`.
            let lowered: String = c.to_lowercase().filter(|c| c.is_alphanumeric()).collect();
            if lowered.is_empty() {
                continue;
            }
            out.push_str(&lowered);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let mut trimmed = out.trim_end_matches('-').to_string();
    if trimmed.len() > max_bytes {
        // Truncate on a character boundary, then on a word boundary if one is
        // close enough that the name still reads.
        let mut cut = max_bytes;
        while cut > 0 && !trimmed.is_char_boundary(cut) {
            cut -= 1;
        }
        trimmed.truncate(cut);
        trimmed = trimmed.trim_end_matches('-').to_string();
    }
    if trimmed.is_empty() {
        "item".into()
    } else {
        trimmed
    }
}

/// Split a comma-separated CLI value into a list, ignoring empties.
pub fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

fn de_string_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let v = Option::<Value>::deserialize(d)?;
    Ok(match v {
        None | Some(Value::Null) => vec![],
        Some(Value::String(s)) => split_list(&s),
        Some(Value::Sequence(seq)) => seq.iter().filter_map(scalar_string).collect(),
        Some(other) => scalar_string(&other).into_iter().collect(),
    })
}

fn de_id_list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u32>, D::Error> {
    let raw = de_string_list(d)?;
    let mut out = Vec::new();
    for s in raw {
        let t = s.trim().trim_start_matches('#');
        match t.parse::<u32>() {
            Ok(n) => out.push(n),
            Err(_) => {
                return Err(serde::de::Error::custom(format!(
                    "`{s}` is not a valid item id"
                )));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_filename_safe() {
        assert_eq!(slug("Add OAuth login!", 240), "add-oauth-login");
        assert_eq!(slug("  Trailing / slashes  ", 240), "trailing-slashes");
        assert_eq!(slug("***", 240), "item");
    }

    /// Lowercasing can lengthen a character into something that is no longer a
    /// letter. Turkish dotted capital I becomes `i` plus a combining dot, and
    /// the mark used to reach the filename -- invisible in a terminal, and one
    /// more thing for a filesystem to normalise differently.
    #[test]
    fn lowercasing_never_smuggles_a_combining_mark_through() {
        assert_eq!(slug("İ", 240), "i");
        assert_eq!(slug("İstanbul rewrite", 240), "istanbul-rewrite");
        assert_eq!(key_from_title("İstanbul"), "istanbul");
        for s in [slug("İ", 240), key_from_title("İ")] {
            assert!(
                s.chars().all(char::is_alphanumeric),
                "a mark survived: {s:?}"
            );
        }
    }

    /// A key is typed at the command line, so it keeps what a version number is
    /// made of. This is the whole difference from `slug`, and getting it wrong
    /// turns `milestone=v1.0` into `milestone=v1-0` for everybody.
    #[test]
    fn a_key_from_a_title_stays_typeable() {
        assert_eq!(key_from_title("v1.0"), "v1.0");
        assert_eq!(key_from_title("Usable in anger"), "usable-in-anger");
        assert_eq!(key_from_title("Q1 2027"), "q1-2027");
        assert_eq!(key_from_title("v2.0 (beta)"), "v2.0-beta");
        assert_eq!(
            key_from_title("  Leading and trailing  "),
            "leading-and-trailing"
        );
        assert_eq!(key_from_title("release_1+2"), "release_1+2");
    }

    /// Empty is not a key. `cairn new` refuses rather than creating an item of a
    /// grouping type with nothing to name it by, which is the bug this exists to
    /// close.
    #[test]
    fn a_title_with_nothing_to_take_yields_no_key() {
        for title in ["***", "   ", "", "...", "---"] {
            assert_eq!(key_from_title(title), "", "{title:?}");
        }
    }

    #[test]
    fn slugs_are_only_shortened_to_fit_the_filesystem() {
        // A long title survives intact when there is room for it.
        let long = "a ".repeat(80);
        assert!(slug(&long, 240).len() > 60);
        // And is cut on a character boundary when there is not.
        let cut = slug("dærlig æøå ".repeat(40).as_str(), 32);
        assert!(cut.len() <= 32);
        assert!(std::str::from_utf8(cut.as_bytes()).is_ok());
    }

    #[test]
    fn line_numbers_point_at_the_offending_field() {
        let src = "---\nid: 1\ntitle: T\nstatus: bogus\n---\nbody\n";
        let item = Item::parse(Path::new("0001-t.md"), src).unwrap();
        assert_eq!(item.line_of("status"), Some(4));
        assert_eq!(item.line_of("id"), Some(2));
        assert_eq!(item.line_of("nonesuch"), None);
    }

    #[test]
    fn frontmatter_splits_on_the_first_closing_delimiter() {
        let text = "---\ntitle: A\n---\n\nBody --- with dashes\n";
        let (front, body) = split_frontmatter(text).unwrap();
        assert_eq!(front, "title: A\n");
        assert_eq!(body, "\nBody --- with dashes\n");
    }

    #[test]
    fn frontmatter_is_required() {
        assert!(split_frontmatter("no frontmatter here").is_none());
        assert!(split_frontmatter("---\nunterminated: true\n").is_none());
    }

    #[test]
    fn labels_accept_a_bare_string_or_a_list() {
        let bare = Item::parse(
            Path::new("0001-x.md"),
            "---\nid: 1\ntitle: X\nlabels: auth, backend\n---\nbody\n",
        )
        .unwrap();
        assert_eq!(bare.meta.labels, vec!["auth", "backend"]);

        let list = Item::parse(
            Path::new("0002-x.md"),
            "---\nid: 2\ntitle: X\nlabels: [auth, backend]\n---\nbody\n",
        )
        .unwrap();
        assert_eq!(list.meta.labels, bare.meta.labels);
    }

    #[test]
    fn id_falls_back_to_the_filename() {
        let item = Item::parse(Path::new("0042-thing.md"), "---\ntitle: T\n---\nbody\n").unwrap();
        assert_eq!(item.id, 42);
    }

    #[test]
    fn round_trips_through_markdown() {
        let src = "---\nid: 7\ntitle: Round trip\ntype: bug\nstatus: doing\npriority: p0\n---\n\nThe body.\n";
        let item = Item::parse(Path::new("0007-round-trip.md"), src).unwrap();
        let out = item.to_markdown().unwrap();
        let again = Item::parse(Path::new("0007-round-trip.md"), &out).unwrap();
        assert_eq!(again.id, 7);
        assert_eq!(again.title(), "Round trip");
        assert_eq!(again.status(), "doing");
        assert_eq!(again.get("priority"), Field::Text("p0".into()));
        assert_eq!(again.body.trim(), "The body.");
    }

    #[test]
    fn unknown_frontmatter_keys_survive_a_rewrite() {
        let src = "---\nid: 1\ntitle: T\ncustom_thing: kept\n---\nbody\n";
        let item = Item::parse(Path::new("0001-t.md"), src).unwrap();
        assert!(item.to_markdown().unwrap().contains("custom_thing: kept"));
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    /// Titles and bodies arrive from three directions — typed by a person,
    /// written by a model, carried in by import — so the generators lean on
    /// exactly the characters that break naive Markdown and YAML handling.
    fn awkward_text() -> impl Strategy<Value = String> {
        prop_oneof![
            "[\\PC]{0,80}",
            "[a-zA-Z0-9 ]{0,40}",
            Just("---".to_string()),
            Just("title: not really".to_string()),
            Just("  leading and trailing  ".to_string()),
            Just("emoji 🎯 and cjk 日本語".to_string()),
            Just("quotes \" ' ` and colons: everywhere".to_string()),
            Just("#!/comment @anchor &alias *star |pipe >fold".to_string()),
            // Control characters are the sharp case: a newline inside a title
            // could close the frontmatter early if it were emitted verbatim.
            Just("line one\nline two".to_string()),
            Just("sneaky\n---\nstatus: done".to_string()),
            Just("tab\there and \r carriage".to_string()),
        ]
    }

    fn item_strategy() -> impl Strategy<Value = Item> {
        (
            1u32..100_000,
            awkward_text(),
            proptest::option::of("[a-z]{1,10}"),
            "[a-z]{1,10}",
            proptest::collection::vec("[a-z0-9-]{1,12}", 0..4),
            awkward_text(),
            prop_oneof![Just(Eol::Lf), Just(Eol::Crlf)],
        )
            .prop_map(|(id, title, kind, status, labels, body, eol)| {
                // Parsing normalises CRLF away, so an item that came off disk
                // never holds one. Generating them would test a state the
                // program cannot reach.
                let title = title.replace("\r\n", "\n");
                let body = body.replace("\r\n", "\n");
                let mut meta = Meta {
                    id: Some(id),
                    title: Some(title),
                    kind,
                    status: Some(status),
                    labels,
                    ..Default::default()
                };
                meta.created = Some("2026-01-01".into());
                Item {
                    id,
                    meta,
                    body,
                    path: PathBuf::from(format!("{id:04}-x.md")),
                    front: String::new(),
                    eol,
                }
            })
    }

    proptest! {
        /// A body given CRLF from outside is written back as clean line
        /// endings, whatever the file itself uses. Getting this wrong writes
        /// `\r\r\n` and eventually flips the whole file.
        #[test]
        fn a_body_carrying_crlf_is_normalised_on_write(
            item in item_strategy(),
            note in "[a-z ]{1,20}",
        ) {
            let mut item = item;
            item.set_body(&format!("first line\r\n{note}\r\nlast line"));
            let rendered = item.to_markdown().unwrap();
            prop_assert!(!rendered.contains("\r\r"), "doubled carriage return");
            if item.eol == Eol::Lf {
                prop_assert!(!rendered.contains('\r'), "a CRLF body leaked into an LF file");
            }
            // And the file still reads back as the ending it had.
            let again = Item::parse(&item.path, &rendered).unwrap();
            prop_assert_eq!(again.eol, item.eol);
        }

        /// The one guarantee the storage format has to make: what cairn writes,
        /// cairn reads back unchanged.
        #[test]
        fn rendering_then_parsing_preserves_the_item(item in item_strategy()) {
            let rendered = item.to_markdown().unwrap();
            let again = Item::parse(&item.path, &rendered)
                .unwrap_or_else(|e| panic!("could not re-read own output: {e}\n{rendered}"));

            prop_assert_eq!(again.id, item.id);
            prop_assert_eq!(again.meta.title, item.meta.title);
            prop_assert_eq!(again.meta.kind, item.meta.kind);
            prop_assert_eq!(again.meta.status, item.meta.status);
            prop_assert_eq!(again.meta.labels, item.meta.labels);
            prop_assert_eq!(again.body.trim_end(), item.body.trim_end());
        }

        /// And writing it a second time is byte-for-byte identical, so an
        /// unchanged item never shows up as a diff.
        #[test]
        fn rendering_is_stable(item in item_strategy()) {
            let once = item.to_markdown().unwrap();
            let reparsed = Item::parse(&item.path, &once).unwrap();
            prop_assert_eq!(reparsed.to_markdown().unwrap(), once);
        }

        /// Line endings survive the round trip, whichever the file used.
        #[test]
        fn line_endings_survive(item in item_strategy()) {
            let rendered = item.to_markdown().unwrap();
            let again = Item::parse(&item.path, &rendered).unwrap();
            prop_assert_eq!(again.eol, item.eol);
            // The claim is about the line breaks cairn emits, not about every
            // byte: a lone carriage return inside a title or body is content,
            // and preserving it is correct.
            if item.eol == Eol::Crlf {
                prop_assert!(
                    !rendered.replace("\r\n", "").contains('\n'),
                    "a CRLF file has no bare newlines"
                );
            } else {
                prop_assert!(
                    !rendered.contains("\r\n"),
                    "an LF file has no CRLF sequences"
                );
            }
        }

        /// The parser is the trust boundary for typed, generated and imported
        /// data alike. It may reject anything; it may never panic.
        #[test]
        fn parsing_arbitrary_input_never_panics(raw in "\\PC*") {
            let _ = Item::parse(Path::new("0001-x.md"), &raw);
        }

        #[test]
        fn parsing_arbitrary_frontmatter_never_panics(front in "\\PC*", body in "\\PC*") {
            let text = format!("---\n{front}\n---\n{body}");
            let _ = Item::parse(Path::new("0001-x.md"), &text);
        }

        /// A key goes into YAML frontmatter unquoted, into a filter expression,
        /// and onto a command line. Every one of those breaks on whitespace, and
        /// the frontmatter breaks on a newline in a way that silently truncates
        /// the item -- so the guarantee is that a key is one plain word or
        /// nothing at all.
        #[test]
        fn a_derived_key_is_always_one_plain_word(title in awkward_text()) {
            let k = key_from_title(&title);
            prop_assert!(k.chars().all(|c| !c.is_whitespace()), "whitespace in {k:?}");
            prop_assert!(!k.contains(['\'', '"', ':', ',', '#', '=']), "needs quoting: {k:?}");
            prop_assert!(!k.starts_with('-') && !k.ends_with('-'), "{k:?}");
            prop_assert!(!k.starts_with('.') && !k.ends_with('.'), "{k:?}");
            // The first character is alphanumeric or there is no key: `+` and
            // `_` are only ever kept after something real.
            prop_assert!(
                k.is_empty() || k.chars().next().is_some_and(char::is_alphanumeric),
                "{k:?}"
            );
            // The same invariant `slug` has, and the same reason: a combining
            // mark is invisible in the terminal and typeable by nobody.
            prop_assert!(
                k.chars().all(|c| c.is_alphanumeric() || matches!(c, '-' | '.' | '_' | '+')),
                "{k:?}"
            );
        }

        #[test]
        fn slugs_are_always_usable_as_filenames(title in awkward_text(), budget in 8usize..250) {
            let s = slug(&title, budget);
            prop_assert!(!s.is_empty());
            prop_assert!(s.len() <= budget, "{} bytes exceeds {}", s.len(), budget);
            prop_assert!(!s.starts_with('-') && !s.ends_with('-'));
            prop_assert!(s.chars().all(|c| c.is_alphanumeric() || c == '-'));
            prop_assert!(std::str::from_utf8(s.as_bytes()).is_ok());
        }
    }
}

#[cfg(test)]
mod criteria_tests {
    use super::*;

    fn item(body: &str) -> Item {
        Item::parse(
            Path::new("0001-x.md"),
            &format!("---\nid: 1\ntitle: X\nstatus: backlog\n---\n{body}"),
        )
        .expect("parses")
    }

    #[test]
    fn counts_ticked_and_unticked() {
        let i = item("- [ ] one\n- [x] two\n- [X] three\n");
        assert_eq!(i.criteria(None), Criteria { done: 2, total: 3 });
    }

    #[test]
    fn an_item_with_no_boxes_states_no_criteria() {
        let i = item("Just prose, and a list:\n\n- a thing\n- another\n");
        let c = i.criteria(None);
        assert!(!c.any(), "prose bullets are not criteria");
        // Vacuously complete, so the common case produces no noise anywhere.
        assert!(c.complete());
    }

    /// The things that look like checkboxes and are not. Getting this wrong
    /// makes the count quietly untrue, which is worse than not counting.
    #[test]
    fn near_misses_are_not_criteria() {
        for body in [
            "-[ ] no space after the marker\n",
            "- [] no space inside\n",
            "- [ x] a space and an x\n",
            "- [y] not a tick\n",
            "- [xx] two of them\n",
            "[ ] no list marker at all\n",
            "- [ ]nothing after, but glued to text\n",
        ] {
            let c = item(body).criteria(None);
            assert!(!c.any(), "counted a checkbox in {body:?}");
        }
    }

    /// The type template cairn ships ends with a bare `- [ ]`, prompting the
    /// author to write a criterion. It is a placeholder, and counting it would
    /// make every item ever created report one unticked criterion for the rest
    /// of its life.
    ///
    /// This test asserted the opposite first, on the reasoning that the
    /// template emits it so it must be meant. Re-recording the README demo is
    /// what showed that backwards: the demo closed an untouched item and the
    /// tool announced "1 of 1 acceptance criteria are unticked".
    #[test]
    fn an_empty_box_is_a_placeholder_rather_than_a_criterion() {
        for body in ["- [ ]\n", "- [x]\n", "- [ ]   \n", "- [x]\t\n"] {
            assert!(
                !item(body).criteria(None).any(),
                "counted an empty box in {body:?}"
            );
        }
        assert_eq!(item("- [ ] a real one\n").criteria(None).total, 1);
    }

    #[test]
    fn indented_and_alternately_marked_boxes_count() {
        let i = item("- [ ] top\n  - [x] nested\n* [ ] star\n+ [x] plus\n");
        assert_eq!(i.criteria(None), Criteria { done: 2, total: 4 });
    }

    #[test]
    fn a_section_restricts_the_count() {
        let body = "## Problem\n\n- [x] not a criterion, just a note\n\n\
                    ## Acceptance criteria\n\n- [ ] one\n- [x] two\n\n\
                    ## Notes\n\n- [ ] also not a criterion\n";
        let i = item(body);
        assert_eq!(
            i.criteria(Some("Acceptance criteria")),
            Criteria { done: 1, total: 2 },
            "only the named section counts"
        );
        assert_eq!(
            i.criteria(None),
            Criteria { done: 2, total: 4 },
            "and without a section, everything does"
        );
    }

    #[test]
    fn a_section_is_matched_case_insensitively_and_at_any_depth() {
        let body = "### ACCEPTANCE CRITERIA\n\n- [ ] one\n";
        assert_eq!(item(body).criteria(Some("Acceptance criteria")).total, 1);
    }

    /// `criteria_list` is what gives the numbers `cairn tick` takes, so it has
    /// to agree with `criteria` about every one of the near misses above.
    #[test]
    fn the_list_and_the_count_agree() {
        let body = "- [ ] one\n- [x]\n- [ ] two\nnot a box\n- [X] three\n";
        let i = item(body);
        let list = i.criteria_list(None);
        assert_eq!(list.len(), i.criteria(None).total);
        assert_eq!(
            list.iter().filter(|c| c.ticked).count(),
            i.criteria(None).done
        );
        assert_eq!(list[0].text, "one", "the marker and box are stripped");
        assert_eq!(list[2].text, "three");
        // The line each one is written on, which is what the write path uses.
        assert_eq!(
            list.iter().map(|c| c.line).collect::<Vec<_>>(),
            vec![0, 2, 4]
        );
    }

    /// The criterion is the sentence that says what done means. Rewriting any
    /// of it is the failure `cairn tick` exists to prevent, so only the three
    /// bytes of the box may move.
    #[test]
    fn only_the_box_is_rewritten() {
        let mut i = item("  - [ ] Indented, with a [bracket] and trailing  \n- [ ] plain\n");
        assert!(i.set_criteria(&[0], true));
        assert_eq!(
            i.body,
            "  - [x] Indented, with a [bracket] and trailing  \n- [ ] plain\n"
        );
        assert!(i.set_criteria(&[0], false));
        assert_eq!(
            i.body,
            "  - [ ] Indented, with a [bracket] and trailing  \n- [ ] plain\n"
        );
    }

    /// Writing what is already written is what a retried script does. Saying so
    /// is how the command knows to leave `updated` alone and fire no hook.
    #[test]
    fn setting_a_box_to_what_it_already_is_reports_no_change() {
        let mut i = item("- [x] done\n");
        let before = i.body.clone();
        assert!(!i.set_criteria(&[0], true));
        assert_eq!(i.body, before);
    }

    /// A body that ended without a newline must not gain one, and a body that
    /// ended with one must not lose it: either is a spurious diff on every tick.
    #[test]
    fn the_bodys_final_newline_survives_either_way() {
        let mut with = item("- [ ] one\n");
        with.set_criteria(&[0], true);
        assert_eq!(with.body, "- [x] one\n");

        let mut without = item("- [ ] one");
        without.set_criteria(&[0], true);
        assert_eq!(without.body, "- [x] one");

        let mut blank = item("- [ ] one\n\n");
        blank.set_criteria(&[0], true);
        assert_eq!(blank.body, "- [x] one\n\n");
    }

    /// A line the caller did not name is not touched, and a line number past
    /// the end is not an error -- the caller resolved the numbers already.
    #[test]
    fn only_the_named_lines_move() {
        let mut i = item("- [ ] one\n- [ ] two\n- [ ] three\n");
        assert!(i.set_criteria(&[1, 99], true));
        assert_eq!(i.body, "- [ ] one\n- [x] two\n- [ ] three\n");
    }

    #[test]
    fn a_missing_section_counts_nothing_rather_than_everything() {
        // The dangerous failure: falling back to counting the whole body would
        // silently include boxes the project meant to exclude.
        let body = "## Problem\n\n- [ ] a box outside the section\n";
        assert!(!item(body).criteria(Some("Acceptance criteria")).any());
    }
}

/// Merge two versions of an item against their common ancestor, or decline.
///
/// Returns `None` when the two sides disagree about something that has no
/// answer, in which case git's conflict markers stay and a person decides.
///
/// The rules, and no others:
///
/// - one side changed a key and the other did not: take the change
/// - both changed a **sequence**: union, ancestor-aware, so a value one side
///   deliberately removed does not come back from the dead
/// - both changed `updated`: the later date, because it is a stamp rather than
///   a statement
///
/// Everything else that differs on both sides is two people saying different
/// things about one fact. Guessing there would lose an edit somebody meant, and
/// the reason this driver is trustworthy for generated files is that it only
/// touches what it could rebuild. An item cannot be rebuilt, so the bar is
/// higher rather than lower.
pub fn merge_three_way(ours: &Item, base: &Item, theirs: &Item) -> Option<Item> {
    if ours.id != theirs.id {
        return None;
    }
    // The body is prose; there is no union of two paragraphs.
    let body = pick(&ours.body, &base.body, &theirs.body)?;

    let mut merged = ours.clone();
    merged.set_body(&body);

    let keys: Vec<Value> = ours
        .meta
        .extra
        .keys()
        .chain(base.meta.extra.keys())
        .chain(theirs.meta.extra.keys())
        .cloned()
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut extra = Mapping::new();
    for key in keys {
        let name = key.as_str().unwrap_or_default().to_string();
        if !seen.insert(name.clone()) {
            continue;
        }
        let get = |m: &Mapping| m.get(&key).cloned();
        let (o, b, x) = (
            get(&ours.meta.extra),
            get(&base.meta.extra),
            get(&theirs.meta.extra),
        );
        match merge_value(o, b, x) {
            Some(Some(v)) => {
                extra.insert(key, v);
            }
            // Both sides removed it, or one removed and the other left it.
            Some(None) => {}
            None => return None,
        }
    }
    merged.meta.extra = extra;

    // The documented keys, each by the same rules.
    merged.meta.title = pick(&ours.meta.title, &base.meta.title, &theirs.meta.title)?;
    merged.meta.key = pick(&ours.meta.key, &base.meta.key, &theirs.meta.key)?;
    merged.meta.kind = pick(&ours.meta.kind, &base.meta.kind, &theirs.meta.kind)?;
    merged.meta.status = pick(&ours.meta.status, &base.meta.status, &theirs.meta.status)?;
    merged.meta.milestone = pick(
        &ours.meta.milestone,
        &base.meta.milestone,
        &theirs.meta.milestone,
    )?;
    merged.meta.assignee = pick(
        &ours.meta.assignee,
        &base.meta.assignee,
        &theirs.meta.assignee,
    )?;
    merged.meta.owner = pick(&ours.meta.owner, &base.meta.owner, &theirs.meta.owner)?;
    merged.meta.created_by = pick(
        &ours.meta.created_by,
        &base.meta.created_by,
        &theirs.meta.created_by,
    )?;
    merged.meta.source = pick(&ours.meta.source, &base.meta.source, &theirs.meta.source)?;
    merged.meta.created = pick(&ours.meta.created, &base.meta.created, &theirs.meta.created)?;

    merged.meta.labels = union(&ours.meta.labels, &base.meta.labels, &theirs.meta.labels);
    merged.meta.depends_on = union(
        &ours.meta.depends_on,
        &base.meta.depends_on,
        &theirs.meta.depends_on,
    );

    // A stamp rather than a statement, so the later one is simply right.
    merged.meta.updated = match (&ours.meta.updated, &theirs.meta.updated) {
        (Some(a), Some(b)) => Some(if a >= b { a.clone() } else { b.clone() }),
        (a, b) => a.clone().or_else(|| b.clone()),
    };
    Some(merged)
}

/// Take whichever side changed, or `None` if both did and they disagree.
fn pick<T: Clone + PartialEq>(ours: &T, base: &T, theirs: &T) -> Option<T> {
    if ours == theirs {
        Some(ours.clone())
    } else if base == ours {
        Some(theirs.clone())
    } else if base == theirs {
        Some(ours.clone())
    } else {
        None
    }
}

/// The union of two sequences against their ancestor.
///
/// Ancestor-aware so that removal survives: a value in the base that one side
/// dropped stays dropped, rather than being restored by the other side simply
/// not having touched it.
fn union<T: Clone + PartialEq>(ours: &[T], base: &[T], theirs: &[T]) -> Vec<T> {
    let removed = |side: &[T]| -> Vec<&T> { base.iter().filter(|v| !side.contains(v)).collect() };
    let gone: Vec<&T> = removed(ours).into_iter().chain(removed(theirs)).collect();

    let mut out: Vec<T> = Vec::new();
    for v in ours.iter().chain(theirs.iter()) {
        if gone.contains(&v) || out.contains(v) {
            continue;
        }
        out.push(v.clone());
    }
    out
}

/// One extra field, three ways. `Some(None)` means "resolved to absent".
fn merge_value(
    ours: Option<Value>,
    base: Option<Value>,
    theirs: Option<Value>,
) -> Option<Option<Value>> {
    if ours == theirs {
        return Some(ours);
    }
    if base == ours {
        return Some(theirs);
    }
    if base == theirs {
        return Some(ours);
    }
    // Both changed. A sequence has an answer; a scalar does not.
    match (&ours, &theirs) {
        (Some(Value::Sequence(o)), Some(Value::Sequence(t))) => {
            let b = match &base {
                Some(Value::Sequence(b)) => b.clone(),
                _ => Vec::new(),
            };
            Some(Some(Value::Sequence(union(o, &b, t))))
        }
        _ => None,
    }
}

/// Items whose titles are close enough to `title` to be worth mentioning.
///
/// An agent starts every session cold, so the backlog is not its record but its
/// memory — and filing a duplicate is therefore not an unlucky mistake but the
/// characteristic failure of an agent using this tool. Three items called
/// `Task 1` appeared in a scratch project without a murmur.
///
/// Deliberately boring: normalise, compare word sets, report anything sharing
/// most of its words. Nothing clever, nothing that needs a model, nothing that
/// can be wrong in an interesting way. It reports and never refuses — two items
/// genuinely called the same thing is a real situation, and a tool that refused
/// would teach people to mangle titles to get past it.
pub fn near_duplicates<'a>(title: &str, items: &'a [Item]) -> Vec<&'a Item> {
    fn words(text: &str) -> std::collections::BTreeSet<String> {
        text.split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .map(str::to_lowercase)
            .collect()
    }

    let wanted = words(title);
    if wanted.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(usize, &Item)> = items
        .iter()
        .filter_map(|i| {
            let theirs = words(i.title());
            if theirs.is_empty() {
                return None;
            }
            let shared = wanted.intersection(&theirs).count();
            // Most of the shorter title's words, so a long title does not match
            // every short one that happens to be a prefix of it.
            let smaller = wanted.len().min(theirs.len());
            (shared * 4 >= smaller * 3).then_some((shared, i))
        })
        .collect();
    // Most alike first, then by id, so the report is stable.
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.id.cmp(&b.1.id)));
    found.into_iter().map(|(_, i)| i).take(3).collect()
}
