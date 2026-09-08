// cairn — the test harness.
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
//
// One harness, shared by every integration test.
//
// There used to be four `Project`s, three `Rng`s, three `path_with_binary`s and
// two `Out`s across six files, and they had drifted: `run` returned `Out` in one
// place, `(i32, String, String)` in another and `(i32, String)` in a third. Any
// improvement to the harness had to be made four times or not at all, so it was
// made once and copied, which is how they diverged.
//
// Two things here are worth reading before writing a test.
//
// `Schema` builds `cairn.toml` from values rather than by string surgery on the
// shipped template. Forty-one `.replace()` calls against that template is what
// this replaces, and they broke constantly: a `[render]` appended twice, a
// `link_items = false` that had moved, a `title = "Roadmap"` that was not where
// the test guessed. A schema built from parts cannot be wrong about the file it
// is editing, because it is not editing one.
//
// The assertions print the difference rather than the haystack. `assert_contains`
// on a five-kilobyte JSON document used to dump all five kilobytes.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// --- finding the binary under test ------------------------------------------

/// The binary this test run is for, from the same profile cargo built.
pub fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_cairn")
}

/// A `PATH` with the binary's directory first, so a hook or a merge driver that
/// runs `cairn` finds the one being tested rather than an installed one.
pub fn path_with_binary() -> std::ffi::OsString {
    let dir = Path::new(bin()).parent().expect("binary directory");
    let existing = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(&existing));
    std::env::join_paths(paths).expect("PATH")
}

// --- what a command did -----------------------------------------------------

#[derive(Debug, Clone)]
pub struct Out {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    pub fn ok(&self) -> bool {
        self.code == 0
    }

    /// Both streams, for an assertion that does not care which one carried it.
    pub fn all(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    pub fn trimmed(&self) -> String {
        self.stdout.trim().to_string()
    }

    pub fn lines(&self) -> Vec<String> {
        self.stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Standard output as JSON, with the output in the failure when it is not.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|e| {
            panic!(
                "expected JSON on standard output: {e}\n{}",
                excerpt(&self.stdout)
            )
        })
    }
}

// --- assertions that say what went wrong ------------------------------------

/// At most this much of a haystack is worth printing. A failure that dumps five
/// kilobytes of JSON is a failure nobody reads.
const EXCERPT: usize = 1200;

pub fn excerpt(text: &str) -> String {
    if text.chars().count() <= EXCERPT {
        return text.to_string();
    }
    let head: String = text.chars().take(EXCERPT).collect();
    format!(
        "{head}\n… {} more characters",
        text.chars().count() - EXCERPT
    )
}

#[track_caller]
pub fn assert_contains(haystack: &str, needle: &str, what: &str) {
    if haystack.contains(needle) {
        return;
    }
    // The nearest line, when there is one, is more use than the whole output:
    // it is almost always a wording change rather than a missing message.
    let near = haystack
        .lines()
        .filter(|l| {
            let l = l.to_lowercase();
            needle
                .split_whitespace()
                .take(3)
                .any(|w| w.len() > 3 && l.contains(&w.to_lowercase()))
        })
        .take(3)
        .collect::<Vec<_>>()
        .join("\n");

    let hint = if near.is_empty() {
        String::new()
    } else {
        format!("\nnearest lines:\n{near}\n")
    };
    panic!(
        "{}: expected to find\n  {needle}\n{hint}\nin:\n{}",
        if what.is_empty() { "assertion" } else { what },
        excerpt(haystack)
    );
}

#[track_caller]
pub fn assert_missing(haystack: &str, needle: &str, why: &str) {
    if !haystack.contains(needle) {
        return;
    }
    let line = haystack
        .lines()
        .find(|l| l.contains(needle))
        .unwrap_or_default();
    panic!("{why}: found `{needle}` in:\n  {line}");
}

/// Compare two line-oriented outputs and print only the lines that differ.
#[track_caller]
pub fn assert_lines_eq(got: &str, want: &str, what: &str) {
    let g: Vec<&str> = got.lines().collect();
    let w: Vec<&str> = want.lines().collect();
    if g == w {
        return;
    }
    let mut diff = String::new();
    for n in 0..g.len().max(w.len()) {
        let a = g.get(n).copied().unwrap_or("<nothing>");
        let b = w.get(n).copied().unwrap_or("<nothing>");
        if a != b {
            diff.push_str(&format!("  line {}:\n    got  {a}\n    want {b}\n", n + 1));
        }
    }
    panic!("{what}:\n{diff}");
}

/// One value inside a JSON document, named by a dotted path, with the document
/// in the failure.
#[track_caller]
pub fn assert_json(doc: &serde_json::Value, path: &str, want: serde_json::Value) {
    let mut at = doc;
    for step in path.split('.') {
        at = match step.parse::<usize>() {
            Ok(n) => &at[n],
            Err(_) => &at[step],
        };
    }
    if *at == want {
        return;
    }
    panic!(
        "`{path}`:\n  got  {at}\n  want {want}\nin:\n{}",
        excerpt(&serde_json::to_string_pretty(doc).unwrap_or_default())
    );
}

// --- a schema built from values ---------------------------------------------

/// A status category, spelled as `cairn.toml` spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Open,
    Active,
    Done,
    Dropped,
}

impl Category {
    fn as_str(self) -> &'static str {
        match self {
            Category::Open => "open",
            Category::Active => "active",
            Category::Done => "done",
            Category::Dropped => "dropped",
        }
    }
}

/// What an agent may do with a field or a status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Write,
    ReadOnly,
    Propose,
}

impl Agent {
    fn as_str(self) -> &'static str {
        match self {
            Agent::Write => "write",
            Agent::ReadOnly => "read-only",
            Agent::Propose => "propose",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Status {
    name: String,
    category: Option<Category>,
    label: Option<String>,
    color: Option<String>,
    board: bool,
    agent: Agent,
}

impl Status {
    pub fn new(name: &str, category: Category) -> Status {
        Status {
            name: name.into(),
            category: Some(category),
            label: None,
            color: None,
            board: true,
            agent: Agent::Write,
        }
    }

    /// A status that declares no category, which is the one default in the file
    /// that can be silently, consequentially wrong.
    pub fn undeclared(name: &str) -> Status {
        Status {
            category: None,
            ..Status::new(name, Category::Open)
        }
    }

    pub fn label(mut self, l: &str) -> Status {
        self.label = Some(l.into());
        self
    }
    pub fn color(mut self, c: &str) -> Status {
        self.color = Some(c.into());
        self
    }
    pub fn off_the_board(mut self) -> Status {
        self.board = false;
        self
    }
    pub fn agent(mut self, a: Agent) -> Status {
        self.agent = a;
        self
    }

    fn to_toml(&self) -> String {
        let mut s = format!("[[status]]\nname = {:?}\n", self.name);
        if let Some(c) = self.category {
            s += &format!("category = {:?}\n", c.as_str());
        }
        if let Some(l) = &self.label {
            s += &format!("label = {l:?}\n");
        }
        if let Some(c) = &self.color {
            s += &format!("color = {c:?}\n");
        }
        if !self.board {
            s += "board = false\n";
        }
        if self.agent != Agent::Write {
            s += &format!("agent = {:?}\n", self.agent.as_str());
        }
        s
    }
}

#[derive(Debug, Clone)]
pub struct Field {
    name: String,
    kind: String,
    values: Vec<String>,
    target: Option<String>,
    by: Option<&'static str>,
    cardinality: Option<&'static str>,
    rollup: bool,
    acyclic: bool,
    inverse: Option<String>,
    default: Option<String>,
    required: bool,
    column: bool,
    agent: Agent,
    description: Option<String>,
}

impl Field {
    fn of(name: &str, kind: &str) -> Field {
        Field {
            name: name.into(),
            kind: kind.into(),
            values: Vec::new(),
            target: None,
            by: None,
            cardinality: None,
            rollup: false,
            acyclic: false,
            inverse: None,
            default: None,
            required: false,
            column: false,
            agent: Agent::Write,
            description: None,
        }
    }

    pub fn text(name: &str) -> Field {
        Field::of(name, "text")
    }
    pub fn date(name: &str) -> Field {
        Field::of(name, "date")
    }
    pub fn number(name: &str) -> Field {
        Field::of(name, "number")
    }
    pub fn boolean(name: &str) -> Field {
        Field::of(name, "bool")
    }
    pub fn list(name: &str) -> Field {
        Field::of(name, "list")
    }

    pub fn choice<const N: usize>(name: &str, values: [&str; N]) -> Field {
        Field {
            values: values.iter().map(|v| (*v).to_string()).collect(),
            ..Field::of(name, "enum")
        }
    }

    /// A field naming another item. `target` is a type name, or `*` for any.
    pub fn reference(name: &str, target: &str) -> Field {
        Field {
            target: Some(target.into()),
            ..Field::of(name, "ref")
        }
    }

    pub fn by_key(mut self) -> Field {
        self.by = Some("key");
        self
    }
    pub fn by_id(mut self) -> Field {
        self.by = Some("id");
        self
    }
    pub fn many(mut self) -> Field {
        self.cardinality = Some("many");
        self
    }
    pub fn rollup(mut self) -> Field {
        self.rollup = true;
        self
    }
    pub fn acyclic(mut self) -> Field {
        self.acyclic = true;
        self
    }
    pub fn inverse(mut self, name: &str) -> Field {
        self.inverse = Some(name.into());
        self
    }
    pub fn default(mut self, v: &str) -> Field {
        self.default = Some(v.into());
        self
    }
    pub fn required(mut self) -> Field {
        self.required = true;
        self
    }
    pub fn column(mut self) -> Field {
        self.column = true;
        self
    }
    pub fn agent(mut self, a: Agent) -> Field {
        self.agent = a;
        self
    }
    pub fn describe(mut self, d: &str) -> Field {
        self.description = Some(d.into());
        self
    }

    fn to_toml(&self) -> String {
        let mut s = format!(
            "[[field]]\nname = {:?}\nkind = {:?}\n",
            self.name, self.kind
        );
        if !self.values.is_empty() {
            let list: Vec<String> = self.values.iter().map(|v| format!("{v:?}")).collect();
            s += &format!("values = [{}]\n", list.join(", "));
        }
        if let Some(t) = &self.target {
            s += &format!("target = {t:?}\n");
        }
        if let Some(b) = self.by {
            s += &format!("by = {b:?}\n");
        }
        if let Some(c) = self.cardinality {
            s += &format!("cardinality = {c:?}\n");
        }
        if self.rollup {
            s += "rollup = true\n";
        }
        if self.acyclic {
            s += "acyclic = true\n";
        }
        if let Some(i) = &self.inverse {
            s += &format!("inverse = {i:?}\n");
        }
        if let Some(d) = &self.default {
            s += &format!("default = {d:?}\n");
        }
        if self.required {
            s += "required = true\n";
        }
        if self.column {
            s += "column = true\n";
        }
        if self.agent != Agent::Write {
            s += &format!("agent = {:?}\n", self.agent.as_str());
        }
        if let Some(d) = &self.description {
            s += &format!("description = {d:?}\n");
        }
        s
    }
}

/// How the roadmap is rendered.
#[derive(Debug, Clone)]
pub struct Render {
    target: String,
    title: Option<String>,
    group_by: String,
    include: Option<String>,
    header: Option<String>,
    footer: Option<String>,
    link_items: bool,
    progress: bool,
    checkbox: bool,
    show_ids: bool,
    group_by_status: bool,
}

impl Default for Render {
    fn default() -> Render {
        Render {
            target: "ROADMAP.md".into(),
            title: None,
            group_by: "milestone".into(),
            include: None,
            header: None,
            footer: None,
            link_items: false,
            progress: true,
            checkbox: true,
            show_ids: true,
            group_by_status: true,
        }
    }
}

impl Render {
    pub fn target(mut self, t: &str) -> Render {
        self.target = t.into();
        self
    }
    pub fn title(mut self, t: &str) -> Render {
        self.title = Some(t.into());
        self
    }
    pub fn group_by(mut self, g: &str) -> Render {
        self.group_by = g.into();
        self
    }
    pub fn include(mut self, expr: &str) -> Render {
        self.include = Some(expr.into());
        self
    }
    pub fn header(mut self, path: &str) -> Render {
        self.header = Some(path.into());
        self
    }
    pub fn footer(mut self, path: &str) -> Render {
        self.footer = Some(path.into());
        self
    }
    pub fn link_items(mut self) -> Render {
        self.link_items = true;
        self
    }

    fn to_toml(&self) -> String {
        let mut s = format!("[render]\ntarget = {:?}\n", self.target);
        if let Some(t) = &self.title {
            s += &format!("title = {t:?}\n");
        }
        s += &format!("group_by = {:?}\n", self.group_by);
        if let Some(i) = &self.include {
            s += &format!("include = {i:?}\n");
        }
        if let Some(h) = &self.header {
            s += &format!("header = {h:?}\n");
        }
        if let Some(f) = &self.footer {
            s += &format!("footer = {f:?}\n");
        }
        s += &format!(
            "link_items = {}\nprogress = {}\ncheckbox = {}\nshow_ids = {}\ngroup_by_status = {}\n",
            self.link_items, self.progress, self.checkbox, self.show_ids, self.group_by_status
        );
        s
    }
}

/// A whole `cairn.toml`, built from parts.
///
/// `Schema::standard()` is close to what `cairn init` writes, and is the base
/// for anything that wants "the usual project, but…". Nothing here edits a
/// file: the configuration is assembled and written once, so a test cannot be
/// wrong about where a key was or how many times a table appears.
#[derive(Debug, Clone)]
pub struct Schema {
    format: Option<u32>,
    name: String,
    description: Option<String>,
    dir: String,
    url: Option<String>,
    id_format: Option<String>,
    id_width: Option<usize>,
    id_start: Option<u32>,
    filename_max: Option<usize>,
    default_type: Option<String>,
    default_status: Option<String>,
    criteria_section: Option<String>,
    require_criteria: bool,
    types: Vec<(String, Option<String>)>,
    statuses: Vec<Status>,
    fields: Vec<Field>,
    views: Vec<(String, String, Option<String>)>,
    hooks: Vec<(String, String)>,
    render: Option<Render>,
    extra: String,
}

impl Schema {
    /// The smallest thing that is still a project: a format, a name, and one
    /// status.
    pub fn bare() -> Schema {
        Schema {
            format: Some(2),
            name: "Testbed".into(),
            description: None,
            dir: "cairn/items".into(),
            url: None,
            id_format: None,
            id_width: None,
            id_start: None,
            filename_max: None,
            default_type: None,
            default_status: None,
            criteria_section: None,
            require_criteria: false,
            types: Vec::new(),
            statuses: vec![Status::new("todo", Category::Open)],
            fields: Vec::new(),
            views: Vec::new(),
            hooks: Vec::new(),
            render: None,
            extra: String::new(),
        }
    }

    /// The shape most tests want: the statuses, types and fields a real project
    /// has, milestones included.
    pub fn standard() -> Schema {
        Schema {
            default_type: Some("feature".into()),
            default_status: Some("backlog".into()),
            types: vec![
                ("feature".into(), None),
                ("bug".into(), None),
                ("chore".into(), None),
                ("docs".into(), None),
                (
                    "milestone".into(),
                    Some("a release, or whatever this ships".into()),
                ),
            ],
            statuses: vec![
                Status::new("backlog", Category::Open),
                Status::new("planned", Category::Open).color("blue"),
                Status::new("doing", Category::Active)
                    .label("in progress")
                    .color("yellow"),
                Status::new("blocked", Category::Active).color("red"),
                Status::new("done", Category::Done).color("green"),
                Status::new("dropped", Category::Dropped).off_the_board(),
            ],
            fields: vec![
                Field::choice("priority", ["p0", "p1", "p2", "p3"])
                    .default("p2")
                    .column(),
                Field::choice("effort", ["s", "m", "l", "xl"]),
                Field::text("area"),
                Field::date("due"),
                Field::reference("milestone", "milestone")
                    .by_key()
                    .rollup()
                    .inverse("scheduled"),
            ],
            render: Some(Render::default()),
            ..Schema::bare()
        }
    }

    pub fn format(mut self, n: u32) -> Schema {
        self.format = Some(n);
        self
    }
    pub fn no_format_key(mut self) -> Schema {
        self.format = None;
        self
    }
    pub fn name(mut self, n: &str) -> Schema {
        self.name = n.into();
        self
    }
    pub fn description(mut self, d: &str) -> Schema {
        self.description = Some(d.into());
        self
    }
    pub fn dir(mut self, d: &str) -> Schema {
        self.dir = d.into();
        self
    }
    pub fn url(mut self, u: &str) -> Schema {
        self.url = Some(u.into());
        self
    }
    pub fn id_format(mut self, f: &str) -> Schema {
        self.id_format = Some(f.into());
        self
    }
    pub fn id_width(mut self, w: usize) -> Schema {
        self.id_width = Some(w);
        self
    }
    pub fn id_start(mut self, n: u32) -> Schema {
        self.id_start = Some(n);
        self
    }
    pub fn filename_max(mut self, n: usize) -> Schema {
        self.filename_max = Some(n);
        self
    }
    pub fn criteria_section(mut self, s: &str) -> Schema {
        self.criteria_section = Some(s.into());
        self
    }
    pub fn require_criteria(mut self) -> Schema {
        self.require_criteria = true;
        self
    }
    #[track_caller]
    pub fn item_type(mut self, name: &str) -> Schema {
        assert!(
            !self.types.iter().any(|(n, _)| n == name),
            "type `{name}` is already declared"
        );
        self.types.push((name.into(), None));
        self
    }
    #[track_caller]
    pub fn status(mut self, s: Status) -> Schema {
        assert!(
            !self.statuses.iter().any(|x| x.name == s.name),
            "status `{}` is already declared; use `.amend_status(\"{}\", …)`",
            s.name,
            s.name
        );
        self.statuses.push(s);
        self
    }
    /// Replace every status, for a project whose workflow is the point.
    pub fn statuses(mut self, s: Vec<Status>) -> Schema {
        self.statuses = s;
        self
    }
    /// Declare a field. Panics on a name already declared: cairn refuses a
    /// duplicate, and failing here names the line that added the second one
    /// rather than the command that later tripped over it.
    #[track_caller]
    pub fn field(mut self, f: Field) -> Schema {
        assert!(
            !self.fields.iter().any(|x| x.name == f.name),
            "field `{}` is already declared; use `.amend(\"{}\", …)` to change it",
            f.name,
            f.name
        );
        self.fields.push(f);
        self
    }
    /// Change a field already declared, by name. Panics if there is no such
    /// field, because a test that meant to change one and silently did not is
    /// worse than a test that stops.
    #[track_caller]
    pub fn amend(mut self, name: &str, f: impl FnOnce(Field) -> Field) -> Schema {
        let at = self
            .fields
            .iter()
            .position(|x| x.name == name)
            .unwrap_or_else(|| panic!("no field named `{name}` to amend"));
        let existing = self.fields.remove(at);
        self.fields.insert(at, f(existing));
        self
    }
    /// The same, for a status.
    #[track_caller]
    pub fn amend_status(mut self, name: &str, f: impl FnOnce(Status) -> Status) -> Schema {
        let at = self
            .statuses
            .iter()
            .position(|x| x.name == name)
            .unwrap_or_else(|| panic!("no status named `{name}` to amend"));
        let existing = self.statuses.remove(at);
        self.statuses.insert(at, f(existing));
        self
    }
    pub fn view(mut self, name: &str, filter: &str) -> Schema {
        self.views.push((name.into(), filter.into(), None));
        self
    }
    pub fn view_grouped(mut self, name: &str, filter: &str, group_by: &str) -> Schema {
        self.views
            .push((name.into(), filter.into(), Some(group_by.into())));
        self
    }
    pub fn hook(mut self, event: &str, command: &str) -> Schema {
        self.hooks.push((event.into(), command.into()));
        self
    }
    pub fn render(mut self, f: impl FnOnce(Render) -> Render) -> Schema {
        self.render = Some(f(self.render.unwrap_or_default()));
        self
    }
    pub fn no_render(mut self) -> Schema {
        self.render = None;
        self
    }
    /// Raw TOML appended at the end, for the shapes this builder deliberately
    /// cannot express — a `[[milestone]]` block from format 1, a key from a
    /// version that does not exist.
    pub fn raw(mut self, toml: &str) -> Schema {
        self.extra.push_str(toml);
        self
    }

    pub fn to_toml(&self) -> String {
        let mut s = String::new();
        if let Some(f) = self.format {
            s += &format!("format = {f}\n\n");
        }
        s += &format!("[project]\nname = {:?}\ndir = {:?}\n", self.name, self.dir);
        if let Some(d) = &self.description {
            s += &format!("description = {d:?}\n");
        }
        if let Some(u) = &self.url {
            s += &format!("url = {u:?}\n");
        }
        if let Some(f) = &self.id_format {
            s += &format!("id_format = {f:?}\n");
        }
        if let Some(w) = self.id_width {
            s += &format!("id_width = {w}\n");
        }
        if let Some(n) = self.id_start {
            s += &format!("id_start = {n}\n");
        }
        if let Some(n) = self.filename_max {
            s += &format!("filename_max = {n}\n");
        }
        if let Some(t) = &self.default_type {
            s += &format!("default_type = {t:?}\n");
        }
        if let Some(st) = &self.default_status {
            s += &format!("default_status = {st:?}\n");
        }
        if let Some(c) = &self.criteria_section {
            s += &format!("criteria_section = {c:?}\n");
        }
        if self.require_criteria {
            s += "require_criteria = true\n";
        }
        for (name, description) in &self.types {
            s += &format!("\n[[type]]\nname = {name:?}\n");
            if let Some(d) = description {
                s += &format!("description = {d:?}\n");
            }
        }
        for st in &self.statuses {
            s.push('\n');
            s += &st.to_toml();
        }
        for f in &self.fields {
            s.push('\n');
            s += &f.to_toml();
        }
        for (name, filter, group_by) in &self.views {
            s += &format!("\n[[view]]\nname = {name:?}\nfilter = {filter:?}\n");
            if let Some(g) = group_by {
                s += &format!("group_by = {g:?}\n");
            }
        }
        if !self.hooks.is_empty() {
            s += "\n[hooks]\n";
            for (event, command) in &self.hooks {
                s += &format!("{event} = {command:?}\n");
            }
        }
        if let Some(r) = &self.render {
            s.push('\n');
            s += &r.to_toml();
        }
        if !self.extra.is_empty() {
            s.push('\n');
            s += &self.extra;
        }
        s
    }
}

// --- a project on disk ------------------------------------------------------

pub struct Project {
    pub dir: tempfile::TempDir,
}

impl Project {
    /// A directory with nothing in it.
    pub fn empty() -> Project {
        Project {
            dir: tempfile::tempdir().expect("temporary directory"),
        }
    }

    /// A project as `cairn init` writes one, without the example item. Use this
    /// when the shipped template is the thing under test; use `with` when it is
    /// only scenery.
    pub fn new() -> Project {
        Project::with_init(&["init", "--bare", "--name", "Testbed"])
    }

    pub fn with_init(args: &[&str]) -> Project {
        let p = Project::empty();
        p.expect(args);
        p
    }

    /// A project whose schema is exactly what was asked for.
    pub fn with(schema: Schema) -> Project {
        let p = Project::empty();
        p.write("cairn.toml", &schema.to_toml());
        std::fs::create_dir_all(p.path(&schema.dir)).expect("item directory");
        p
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root().join(rel)
    }

    // --- running the binary --------------------------------------------------

    pub fn run(&self, args: &[&str]) -> Out {
        self.run_in(self.root(), args)
    }

    pub fn run_in(&self, cwd: &Path, args: &[&str]) -> Out {
        self.spawn(cwd, args, &[], None)
    }

    /// Run with extra environment. `None` removes a variable.
    pub fn run_env(&self, args: &[&str], env: &[(&str, Option<&str>)]) -> Out {
        self.spawn(self.root(), args, env, None)
    }

    /// Run with something on standard input.
    pub fn run_stdin(&self, args: &[&str], input: &str) -> Out {
        self.spawn(self.root(), args, &[], Some(input))
    }

    /// Run as an agent would, the way the protocol server does.
    pub fn run_as_agent(&self, args: &[&str]) -> Out {
        self.run_env(args, &[("CAIRN_AGENT", Some("claude"))])
    }

    fn spawn(
        &self,
        cwd: &Path,
        args: &[&str],
        env: &[(&str, Option<&str>)],
        input: Option<&str>,
    ) -> Out {
        let mut c = Command::new(bin());
        c.args(args)
            .current_dir(cwd)
            // Deterministic: no colour, a known identity, and hooks left on so
            // the hook tests can exercise them.
            .env("NO_COLOR", "1")
            .env("CAIRN_USER", "tester")
            .env("PATH", path_with_binary())
            .env_remove("CAIRN_NO_HOOKS")
            .env_remove("CAIRN_AGENT")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            match v {
                Some(v) => c.env(k, v),
                None => c.env_remove(k),
            };
        }

        let out = match input {
            None => {
                c.stdin(Stdio::null());
                c.output()
            }
            Some(text) => {
                use std::io::Write;
                c.stdin(Stdio::piped());
                let mut child = c.spawn().unwrap_or_else(|e| panic!("spawning cairn: {e}"));
                child
                    .stdin
                    .take()
                    .expect("stdin")
                    .write_all(text.as_bytes())
                    .expect("writing stdin");
                child.wait_with_output()
            }
        }
        .unwrap_or_else(|e| panic!("running cairn {args:?}: {e}"));

        Out {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    #[track_caller]
    pub fn expect(&self, args: &[&str]) -> Out {
        let out = self.run(args);
        assert!(
            out.ok(),
            "cairn {args:?} failed with {}:\n{}",
            out.code,
            excerpt(&out.all())
        );
        out
    }

    #[track_caller]
    pub fn fails(&self, args: &[&str]) -> Out {
        let out = self.run(args);
        assert!(
            !out.ok(),
            "cairn {args:?} unexpectedly succeeded:\n{}",
            excerpt(&out.all())
        );
        out
    }

    #[track_caller]
    pub fn expect_as_agent(&self, args: &[&str]) -> Out {
        let out = self.run_as_agent(args);
        assert!(out.ok(), "cairn {args:?} failed:\n{}", excerpt(&out.all()));
        out
    }

    #[track_caller]
    pub fn fails_as_agent(&self, args: &[&str]) -> Out {
        let out = self.run_as_agent(args);
        assert!(
            !out.ok(),
            "cairn {args:?} should have failed:\n{}",
            excerpt(&out.all())
        );
        out
    }

    /// Standard output parsed as JSON, with the command in the failure.
    #[track_caller]
    pub fn json(&self, args: &[&str]) -> serde_json::Value {
        let out = self.expect(args);
        serde_json::from_str(&out.stdout)
            .unwrap_or_else(|e| panic!("cairn {args:?} is not JSON: {e}\n{}", excerpt(&out.stdout)))
    }

    // --- shorthands ----------------------------------------------------------

    /// Create an item and return its identifier as printed.
    pub fn add(&self, title: &str, extra: &[&str]) -> String {
        let mut args = vec!["new", title, "-q"];
        args.extend_from_slice(extra);
        self.expect(&args).trimmed()
    }

    /// Create a milestone with a key, which is how anything refers to one.
    pub fn milestone(&self, key: &str, due: Option<&str>) -> String {
        let id = self
            .expect(&["new", key, "-t", "milestone", "-q"])
            .trimmed();
        self.expect(&["set", &id, &format!("key={key}")]);
        if let Some(d) = due {
            self.expect(&["set", &id, &format!("due={d}")]);
        }
        id
    }

    pub fn count(&self) -> usize {
        self.expect(&["list", "--count"])
            .trimmed()
            .parse()
            .unwrap_or(0)
    }

    pub fn count_all(&self) -> usize {
        self.expect(&["list", "-A", "--count"])
            .trimmed()
            .parse()
            .unwrap_or(0)
    }

    /// How many *open* items match. Deliberately not `--all`: this is what
    /// every caller has always meant by it, and widening it here would change
    /// thirty assertions at once without any of them saying so.
    #[track_caller]
    pub fn count_of(&self, filter: &str) -> usize {
        self.expect(&["list", "--filter", filter, "--count"])
            .trimmed()
            .parse()
            .expect("a number")
    }

    /// The same, counting everything.
    #[track_caller]
    pub fn count_of_all(&self, filter: &str) -> usize {
        self.expect(&["list", "-A", "--filter", filter, "--count"])
            .trimmed()
            .parse()
            .expect("a number")
    }

    /// The identifiers a filter selects, as numbers, for the set algebra the
    /// filter laws check.
    pub fn matching(&self, expr: &str) -> BTreeSet<u32> {
        self.expect(&["list", "-A", "--ids", "--filter", expr])
            .lines()
            .iter()
            .filter_map(|l| l.trim().trim_start_matches('0').parse().ok())
            .collect()
    }

    /// Every identifier in the project, as numbers.
    pub fn every_id(&self) -> BTreeSet<u32> {
        self.expect(&["list", "-A", "--ids"])
            .lines()
            .iter()
            .filter_map(|l| l.trim().trim_start_matches('0').parse().ok())
            .collect()
    }

    pub fn ids(&self) -> Vec<String> {
        self.expect(&["list", "-A", "--ids"]).lines()
    }

    // --- the filesystem ------------------------------------------------------

    pub fn write(&self, rel: &str, contents: &str) {
        let path = self.path(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent directory");
        }
        std::fs::write(path, contents).expect("writing");
    }

    /// Write an item file directly, for the shapes no command produces.
    pub fn write_item(&self, name: &str, frontmatter: &str, body: &str) {
        self.write(
            &format!("cairn/items/{name}"),
            &format!("---\n{}\n---\n{body}", frontmatter.trim()),
        );
    }

    pub fn append(&self, rel: &str, extra: &str) {
        let existing = self.read(rel);
        self.write(rel, &(existing + extra));
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).unwrap_or_default()
    }

    pub fn remove(&self, rel: &str) {
        let _ = std::fs::remove_file(self.path(rel));
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.path(rel).exists()
    }

    /// Filenames in a directory, sorted — for asserting a command moved nothing.
    pub fn files(&self, rel: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.path(rel))
            .map(|d| {
                d.flatten()
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// The file behind an item, by identifier.
    pub fn item_file(&self, id: &str) -> String {
        self.read(&self.expect(&["show", id, "--path"]).trimmed())
    }

    /// Replace the whole schema. Every key is written from parts, so nothing
    /// here can miss because the template moved.
    pub fn set_schema(&self, schema: Schema) {
        self.write("cairn.toml", &schema.to_toml());
    }

    pub fn set_hooks(&self, body: &str) {
        let existing = self.read("cairn.toml");
        let without = existing
            .split("[hooks]")
            .next()
            .unwrap_or(&existing)
            .to_string();
        self.write("cairn.toml", &format!("{without}\n[hooks]\n{body}\n"));
    }
}

impl Default for Project {
    fn default() -> Project {
        Project::new()
    }
}

// --- one random number generator --------------------------------------------

/// A small deterministic generator, seeded and reported.
///
/// Three copies of this existed, with the same three methods and `pick`
/// spelled differently in each. Randomised tests are only worth having if a
/// failure can be replayed, which means one generator whose seed is printed.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// Seeded from the environment when it is set, so continuous integration
    /// can vary it and a failure can be reproduced by passing it back.
    pub fn from_env(var: &str, fallback: u64) -> Rng {
        let seed = std::env::var(var)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(fallback);
        println!("{var}={seed}");
        Rng::new(seed)
    }

    /// splitmix64, which is what all four copies of this used. Keeping the
    /// sequence identical matters: seeds recorded in continuous integration and
    /// in failure reports name a particular run, and a different generator
    /// would quietly make every one of them mean something else.
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> Option<&'a T> {
        if items.is_empty() {
            return None;
        }
        let n = self.below(items.len());
        items.get(n)
    }

    /// The same, for a slice known not to be empty.
    #[track_caller]
    pub fn choose<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        self.pick(items).expect("something to choose from")
    }

    pub fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in.max(1)) == 0
    }
}

// --- speaking the agent protocol --------------------------------------------

impl Project {
    /// Drive the protocol server and return one parsed reply per line.
    ///
    /// The session was hand-rolled in two files and half-rolled in a third.
    /// It belongs here: a test about what a tool *does* should not also be a
    /// test of whether somebody remembered to close standard input.
    pub fn mcp(&self, requests: &[&str]) -> Vec<serde_json::Value> {
        let out = self.run_stdin(&["mcp"], &format!("{}\n", requests.join("\n")));
        assert!(out.ok(), "cairn mcp exited {}:\n{}", out.code, out.stderr);
        out.stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                serde_json::from_str(l)
                    .unwrap_or_else(|e| panic!("mcp emitted a non-JSON line: {e}\n{l}"))
            })
            .collect()
    }

    /// An `initialize` naming a client, then one tool call. `None` skips the
    /// handshake, which is what a client that calls a tool first does.
    pub fn mcp_call(
        &self,
        tool: &str,
        args: serde_json::Value,
        client: Option<&str>,
    ) -> serde_json::Value {
        let call = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": tool, "arguments": args }
        })
        .to_string();

        let replies = match client {
            Some(who) => {
                let init = serde_json::json!({
                    "jsonrpc": "2.0", "id": 0, "method": "initialize",
                    "params": { "clientInfo": { "name": who } }
                })
                .to_string();
                self.mcp(&[&init, &call])
            }
            None => self.mcp(&[&call]),
        };
        replies.last().expect("a reply")["result"].clone()
    }

    /// The tools the server advertises.
    pub fn mcp_tools(&self) -> Vec<serde_json::Value> {
        let replies = self.mcp(&[r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#]);
        replies[0]["result"]["tools"]
            .as_array()
            .expect("a list of tools")
            .clone()
    }
}

/// Whether a tool call reported failure in band, which is how the protocol
/// wants a tool to fail.
pub fn refused(result: &serde_json::Value) -> bool {
    result["isError"] == serde_json::json!(true)
}

/// The text a tool call came back with.
pub fn tool_text(result: &serde_json::Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

// --- the harness, checked against the program -------------------------------

#[cfg(test)]
mod harness {
    use super::*;

    /// A builder that produces a configuration cairn rejects is worse than no
    /// builder: every test using it would fail for a reason none of them is
    /// about. So the shapes it offers are checked against the real program.
    #[test]
    fn every_shape_this_builder_offers_is_one_cairn_accepts() {
        for (what, schema) in [
            ("bare", Schema::bare()),
            ("standard", Schema::standard()),
            (
                "everything",
                Schema::standard()
                    .name("Everything")
                    .description("A project with one of each.")
                    .url("https://example.invalid/blob/main")
                    .id_format("MP-{n:04}")
                    .id_start(100)
                    .filename_max(120)
                    .criteria_section("Acceptance criteria")
                    .require_criteria()
                    .item_type("epic")
                    .status(Status::new("shipped", Category::Done).color("green"))
                    .status(Status::undeclared("vague"))
                    .amend("area", |f| f.column())
                    .field(Field::choice("size", ["s", "m", "l"]).default("m"))
                    .field(Field::date("target"))
                    .field(Field::number("points"))
                    .field(Field::boolean("urgent"))
                    .field(Field::list("platforms"))
                    .field(
                        Field::reference("epic", "epic")
                            .by_key()
                            .rollup()
                            .acyclic()
                            .inverse("contains"),
                    )
                    .field(Field::reference("related", "*").by_id().many())
                    .field(Field::text("locked").agent(Agent::ReadOnly))
                    .view("hot", "priority=p0")
                    .view_grouped("by-epic", "category!=done", "epic")
                    .hook("after-create", "true")
                    .render(|r| r.title("The plan").group_by("epic").link_items()),
            ),
        ] {
            let p = Project::with(schema);
            let out = p.run(&["config"]);
            assert!(
                out.ok(),
                "`{what}` is not a schema cairn accepts:\n{}",
                out.all()
            );
            assert!(
                p.run(&["check"]).ok(),
                "`{what}` produced a project that does not validate:\n{}",
                p.run(&["check"]).all()
            );
        }
    }

    /// The standard schema is the base for most tests, so what it declares is
    /// worth pinning: a test that says "the usual project, but…" should not
    /// have to wonder what the usual project is.
    #[test]
    fn the_standard_schema_declares_what_tests_assume() {
        let p = Project::with(Schema::standard());
        let shown = p.expect(&["config"]).stdout;
        for expected in [
            "backlog",
            "planned",
            "doing",
            "blocked",
            "done",
            "dropped",
            "feature",
            "bug",
            "chore",
            "docs",
            "milestone",
            "priority",
            "effort",
            "area",
            "due",
        ] {
            assert_contains(&shown, expected, "the standard schema");
        }

        // And it is a project you can actually work in.
        p.add("Something", &[]);
        p.milestone("v0.1", Some("2026-12-01"));
        p.expect(&["set", "1", "milestone=v0.1"]);
        assert!(p.run(&["check"]).ok());
    }

    /// Amending names a field that has to exist, because a test that meant to
    /// change one and silently did not is worse than a test that stops.
    #[test]
    #[should_panic(expected = "no field named `nonesuch`")]
    fn amending_a_field_that_is_not_there_stops() {
        let _ = Schema::standard().amend("nonesuch", |f| f.required());
    }

    /// cairn refuses a duplicate field, and finding out at `cairn config` means
    /// reading a failure about a file nobody wrote by hand. The builder says it
    /// at the line that added the second one.
    #[test]
    #[should_panic(expected = "field `priority` is already declared")]
    fn declaring_a_field_twice_stops() {
        let _ = Schema::standard().field(Field::text("priority"));
    }

    /// The generator has to replay, or a seed in a failure report means
    /// nothing.
    #[test]
    fn the_same_seed_replays_the_same_run() {
        let a: Vec<u64> = (0..8).map(|_| Rng::new(42).next()).collect();
        let mut r = Rng::new(42);
        let b: Vec<u64> = (0..8).map(|_| r.next()).collect();
        assert_eq!(a[0], b[0], "a fresh generator starts the same way");
        assert_ne!(b[0], b[1], "and does not stand still");

        let mut one = Rng::new(7);
        let mut two = Rng::new(7);
        assert_eq!(
            (0..32).map(|_| one.next()).collect::<Vec<_>>(),
            (0..32).map(|_| two.next()).collect::<Vec<_>>(),
        );
    }

    /// The assertions exist to say what went wrong; a five-kilobyte haystack
    /// says nothing.
    #[test]
    fn a_failure_message_is_readable() {
        let long = "x".repeat(50_000);
        let shown = excerpt(&long);
        assert!(
            shown.len() < 2_000,
            "an excerpt is not an excerpt: {}",
            shown.len()
        );
        assert_contains(&shown, "more characters", "it says what it left out");
    }
}
