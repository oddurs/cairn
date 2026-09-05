// cairn — a markdown-native roadmap and issue manager that lives in your repo.
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
mod cmd;
mod config;
mod filter;
mod hooks;
mod interchange;
mod item;
mod lock;
mod render;
mod store;
mod style;
mod table;

use anyhow::Result;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// `--version` output, in the form the GNU Coding Standards require: package
/// and version, copyright, licence, and the warranty disclaimer.
///
/// clap prints the package name ahead of this, so it begins with the version.
const VERSION_NOTICE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\n",
    "Copyright (C) 2026 Oddur Sigurdsson\n",
    "License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>.\n",
    "This is free software: you are free to change and redistribute it.\n",
    "There is NO WARRANTY, to the extent permitted by law.\n",
);

#[derive(Parser)]
#[command(
    name = "cairn",
    version,
    long_version = VERSION_NOTICE,
    about = "Markdown-native roadmap and issue manager that lives in your repository",
    long_about = "cairn keeps a project's roadmap and issues as plain Markdown files inside the \
repository itself, under a schema you define in cairn.toml. Items are readable in a text editor, \
reviewable in a pull request, and manipulable from the command line — so humans and coding agents \
write into the same structure instead of inventing their own.",
    propagate_version = true,
    disable_help_subcommand = true
)]
struct Cli {
    /// Control colour output
    #[arg(long, global = true, value_name = "WHEN", default_value = "auto")]
    color: ColorWhen,

    /// Run as if cairn was started in DIR
    #[arg(short = 'C', long = "directory", global = true, value_name = "DIR")]
    directory: Option<PathBuf>,

    /// Do not run the hooks configured in cairn.toml
    #[arg(long, global = true, action = ArgAction::SetTrue)]
    no_hooks: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum ColorWhen {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand)]
enum Command {
    /// Create cairn.toml and the item directory
    Init(cmd::init::Args),

    /// Create a new item
    #[command(visible_alias = "add")]
    New(cmd::new::Args),

    /// List items
    #[command(visible_alias = "ls")]
    List(cmd::list::Args),

    /// Show what is ready to work on right now
    Next(cmd::next::Args),

    /// Search titles, bodies and labels
    #[command(visible_alias = "grep")]
    Search(cmd::search::Args),

    /// Take an item: assign it to yourself and start it
    Claim(cmd::claim::ClaimArgs),

    /// Give an item back: clear the assignee and stop work
    Release(cmd::claim::ReleaseArgs),

    /// Show one item in full
    Show(cmd::show::Args),

    /// Change fields on an item: `cairn set 12 status=doing priority=p0`
    Set(cmd::set::Args),

    /// Add to an item's body: `cairn note 12 "Dropped: too costly"`
    Note(cmd::note::Args),

    /// Move items to the first `done` status
    Close(cmd::set::CloseArgs),

    /// Move items back to the default open status
    Reopen(cmd::set::ReopenArgs),

    /// Open an item in $EDITOR
    Edit(cmd::show::EditArgs),

    /// Delete items
    #[command(visible_alias = "rm")]
    Remove(cmd::show::RemoveArgs),

    /// Print a kanban board grouped by status
    Board(cmd::board::Args),

    /// Print the milestone roadmap with progress
    Roadmap(cmd::roadmap::Args),

    /// Generate the Markdown roadmap file
    Render(cmd::render_cmd::Args),

    /// Write the backlog out in the interchange format
    Export(cmd::export::Args),

    /// Read items in from another tracker
    Import(cmd::import::Args),

    /// Validate every item against the schema
    Check(cmd::check::Args),

    /// Repair duplicate item ids, rewriting references
    Renumber(cmd::renumber::Args),

    /// Bring a project up to the current on-disk format
    Migrate(cmd::migrate::Args),

    /// Inspect and manage milestones
    Milestone(cmd::milestone::Args),

    /// Show the resolved configuration
    Config(cmd::misc::ConfigArgs),

    /// Print usage instructions for coding agents (AGENTS.md / CLAUDE.md)
    Agent(cmd::misc::AgentArgs),

    /// Serve the backlog to agents over the Model Context Protocol (stdio)
    Mcp(cmd::mcp::Args),

    /// Generate a shell completion script
    Completions(cmd::misc::CompletionsArgs),

    /// Generate the man page
    Man(cmd::misc::ManArgs),
}

fn main() {
    let cli = Cli::parse();
    if cli.no_hooks {
        hooks::disable();
    }
    style::init(match cli.color {
        ColorWhen::Auto => None,
        ColorWhen::Always => Some(true),
        ColorWhen::Never => Some(false),
    });

    if let Some(dir) = &cli.directory
        && let Err(e) = std::env::set_current_dir(dir)
    {
        eprintln!("{}: {}: {e}", style::red("cairn"), dir.display());
        std::process::exit(2);
    }

    match run(cli.command) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("{}: {e:#}", style::red("cairn"));
            std::process::exit(1);
        }
    }
}

/// Commands return an exit code so `check` and `render --check` can fail CI
/// without dressing a clean "found problems" result up as an internal error.
fn run(command: Command) -> Result<i32> {
    match command {
        Command::Init(a) => cmd::init::run(a),
        Command::New(a) => cmd::new::run(a),
        Command::List(a) => cmd::list::run(a),
        Command::Next(a) => cmd::next::run(a),
        Command::Search(a) => cmd::search::run(a),
        Command::Claim(a) => cmd::claim::claim(a),
        Command::Release(a) => cmd::claim::release(a),
        Command::Show(a) => cmd::show::run(a),
        Command::Set(a) => cmd::set::run(a),
        Command::Note(a) => cmd::note::run(a),
        Command::Close(a) => cmd::set::close(a),
        Command::Reopen(a) => cmd::set::reopen(a),
        Command::Edit(a) => cmd::show::edit(a),
        Command::Remove(a) => cmd::show::remove(a),
        Command::Board(a) => cmd::board::run(a),
        Command::Roadmap(a) => cmd::roadmap::run(a),
        Command::Render(a) => cmd::render_cmd::run(a),
        Command::Export(a) => cmd::export::run(a),
        Command::Import(a) => cmd::import::run(a),
        Command::Check(a) => cmd::check::run(a),
        Command::Renumber(a) => cmd::renumber::run(a),
        Command::Migrate(a) => cmd::migrate::run(a),
        Command::Milestone(a) => cmd::milestone::run(a),
        Command::Config(a) => cmd::misc::config(a),
        Command::Agent(a) => cmd::misc::agent(a),
        Command::Mcp(a) => cmd::mcp::run(a),
        Command::Completions(a) => cmd::misc::completions::<Cli>(a),
        Command::Man(a) => cmd::misc::man::<Cli>(a),
    }
}

/// Shared by `new` and `set`: parse `key=value`, `key+=value`, `key-=value`.
pub fn parse_assignment(s: &str) -> Result<(String, Assign)> {
    for (token, mk) in [
        ("+=", Assign::Add as fn(String) -> Assign),
        ("-=", Assign::Remove as fn(String) -> Assign),
    ] {
        if let Some(pos) = s.find(token) {
            let key = s[..pos].trim().to_string();
            let value = s[pos + token.len()..].trim().to_string();
            anyhow::ensure!(!key.is_empty(), "`{s}`: missing field name");
            return Ok((key, mk(value)));
        }
    }
    match s.find('=') {
        Some(pos) => {
            let key = s[..pos].trim().to_string();
            let value = s[pos + 1..].trim().to_string();
            anyhow::ensure!(!key.is_empty(), "`{s}`: missing field name");
            Ok((key, Assign::Set(value)))
        }
        None => anyhow::bail!("`{s}`: expected field=value (or field+=value / field-=value)"),
    }
}

#[derive(Debug, Clone)]
pub enum Assign {
    Set(String),
    Add(String),
    Remove(String),
}

/// `--quiet` is spelled the same way everywhere.
#[derive(clap::Args)]
pub struct Quiet {
    /// Print only machine-relevant output
    #[arg(short, long, global = false, action = ArgAction::SetTrue)]
    pub quiet: bool,
}
