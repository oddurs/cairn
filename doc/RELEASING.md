# Releasing cairn

Everything here is either automated or a decision somebody has to make. The
automated parts are named so you can tell when one has not run.

## Before the tag

```sh
make check            # fmt, clippy, the whole suite, and cairn's own roadmap
make soak             # a long arbitrary sequence, plus concurrent writers
make fuzz             # 20,000 arbitrary argument vectors
make conformance      # the second reader against the golden corpus
make dist             # the source tarball, built the way CI will build it
```

Then, by hand:

- [ ] `NEWS` has a section for this version, and it describes what a *user*
      would notice rather than what changed in the code.
- [ ] The version in `Cargo.toml` matches the tag you are about to make.
- [ ] `cairn --version` prints it, and the copyright year is current.
- [ ] `AUTHORS` includes everyone who contributed since the last release.
- [ ] If anything the README demo shows has changed, `make demo` was re-run and
      the SVG committed. CI checks this, so a surprise here means CI has not run.
- [ ] If the on-disk format changed, `spec/README.md` changed first, and there is
      a format number and a migration. See "Compatibility" in the manual.
- [ ] If a documented JSON field, exit code or MCP tool was removed or renamed,
      this is a **major** release. `tests/stability.rs` will have told you.

## The tag

```sh
git tag -s v0.1.0 -m 'cairn 0.1.0'      # -s once there is a signing key
git push origin v0.1.0
```

That is the whole of it. The release workflow builds a binary for every
supported platform, builds the source tarball, attests build provenance, signs
everything if `GPG_PRIVATE_KEY` is set, and publishes to crates.io if
`CARGO_REGISTRY_TOKEN` is set. Both are skipped silently when absent, so a fork
can tag without the workflow failing.

## After the release

- [ ] Download one artefact on a machine that did not build it and verify it,
      following the manual's "Verifying a release" chapter as written. If the
      instructions do not work, they are wrong.
- [ ] `cargo install cairn-md` from a clean machine, and check the binary is
      called `cairn`.

### Downstream packages

Each of these is a separate place the version is written down, and each one that
is not updated becomes somebody installing a stale cairn and reporting a bug
that was fixed months ago.

| Where | What to do | Who can |
| --- | --- | --- |
| crates.io | Automatic, if `CARGO_REGISTRY_TOKEN` is set | maintainer |
| Homebrew tap (`Formula/cairn.rb`) | Update `url`, `sha256` and `version` | maintainer |
| AUR | Bump `pkgver`, refresh `.SRCINFO` | package owner |
| nixpkgs | Bump `version` and `cargoHash`, open a pull request | anyone |
| Homebrew core | Only once the notability bar is met | anyone |
| Debian, Fedora | Needs the source tarball and a signature | a distribution packager |

None of these is done yet, and `0053` tracks them. They are listed here now
because the checklist is the thing that stops the list existing only in
somebody's memory.

## If a release goes wrong

Do not delete the tag. A tag somebody may have fetched is published, and moving
it means two people can have different code under one version — which is the
failure this whole document exists to avoid. Release a patch version instead,
and say in `NEWS` what was wrong with the one before it.
