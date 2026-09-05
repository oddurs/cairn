# Security policy

## Reporting a vulnerability

Please report privately through GitHub's
[private vulnerability reporting](https://github.com/oddurs/cairn/security/advisories/new)
rather than opening a public issue.

You should get an acknowledgement within a week. If a fix is warranted, expect a
release within thirty days of confirmation, and credit in `NEWS` unless you
would rather not be named.

## Scope

cairn reads and writes files in a repository and runs programs the project
configures. Things worth reporting:

- Reading or writing outside the project directory from crafted item files,
  configuration, or an interchange document.
- Executing something the project did not configure, or executing a configured
  hook with input it should not have received.
- A crafted item file, `cairn.toml`, or import document that causes memory
  unsafety.
- The MCP server acting outside the project it was pointed at.

Out of scope, because they are how the tool is designed to work:

- Hooks run programs. A project that configures a hook has asked for that
  program to run; the same is true of `$EDITOR` and of `gh` during a GitHub
  import.
- `cairn` trusts the repository it is pointed at. Opening an untrusted
  repository is equivalent to running its build scripts.

## Supported versions

Until 1.0, only the latest release is supported.
