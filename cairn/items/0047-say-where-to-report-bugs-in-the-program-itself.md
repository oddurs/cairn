---
id: 47
title: Say where to report bugs, in the program itself
type: bug
status: backlog
milestone: v0.2
created: 2026-09-05
updated: 2026-09-05
priority: p1
effort: s
---

## Problem

`cairn --help` ends with `-V, --version`. The GNU Coding Standards say a
program should tell you where to report a bug, and this one does not — the
address exists only in files you have to already be reading.

Somebody hitting a problem has the program in front of them and no idea where
to take it. That is a small thing that makes software feel unowned.

## Proposal

A closing section on `--help`:

    Report bugs to: https://github.com/oddurs/cairn/issues
    cairn home page: https://oddurs.github.io/cairn
    General help using GNU software: https://www.gnu.org/gethelp/

The same in the manual page, which clap_mangen already emits from the same
strings, and a `--bug-report` flag that prints the environment a maintainer
will ask for anyway: version, platform, format version, item count.

## Acceptance criteria

- [ ] `--help` names the address, in the GNU shape
- [ ] The manual page carries it too
- [ ] `cairn --bug-report` prints something worth pasting into an issue
