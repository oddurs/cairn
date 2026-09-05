// cairn — the item: one Markdown file with YAML frontmatter.
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
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub milestone: Option<String>,
    #[serde(default)]
    pub assignee: Option<String>,
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
    pub fn get(&self, key: &str) -> Field {
        match key {
            "id" => Field::Text(self.id.to_string()),
            "title" => opt(self.meta.title.as_deref()),
            "type" | "kind" => opt(self.meta.kind.as_deref()),
            "status" => opt(self.meta.status.as_deref()),
            "milestone" => opt(self.meta.milestone.as_deref()),
            "assignee" => opt(self.meta.assignee.as_deref()),
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

    pub fn parse(path: &Path, text: &str) -> Result<Item> {
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
        let id = meta.id.or_else(|| id_from_filename(path)).ok_or_else(|| {
            anyhow::anyhow!(
                "{}: no `id:` in frontmatter and filename does not start with a number",
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
            for lc in c.to_lowercase() {
                out.push(lc);
            }
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

/// Parse a user-supplied id: accepts `12`, `0012`, `#12`.
pub fn parse_id(s: &str) -> Result<u32> {
    let t = s.trim().trim_start_matches('#');
    match t.parse::<u32>() {
        Ok(n) => Ok(n),
        Err(_) => bail!("`{s}` is not a valid item id (expected a number like 12 or 0012)"),
    }
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
