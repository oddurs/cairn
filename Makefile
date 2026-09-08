# cairn — convenience targets. The build itself is cargo's job; this exists for
# the pieces cargo does not cover: documentation, completions, and installing
# them where a system expects to find them.
PREFIX      ?= /usr/local
BINDIR      ?= $(PREFIX)/bin
MANDIR      ?= $(PREFIX)/share/man/man1
INFODIR     ?= $(PREFIX)/share/info
CARGO       ?= cargo
MAKEINFO    ?= makeinfo
CAIRN       := target/release/cairn

.PHONY: all build check test soak fuzz durability coverage conformance audit dist doc info html pdf record demo install install-bin \
        install-man install-info clean roadmap

all: build doc

build:
	$(CARGO) build --release

# Everything CI runs.
check: test
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets -- -D warnings
	$(CAIRN) check --render --strict

# A second reader, written from the specification alone, run over the corpus
# cairn's own tests use. Needs `pip install pyyaml`.
conformance:
	python3 spec/conformance.py

# Advisories, licences and sources. Needs `cargo install cargo-deny`.
audit:
	$(CARGO) deny check

# One entry point. The end-to-end suite drives the built binary from Rust, so
# it runs identically on every supported platform.
test:
	$(CARGO) test

# The demo runs the real commands and renders their real output, so it cannot
# drift from the program.
# Drives cairn through a long random sequence of ordinary operations, checking
# after every step that the backlog still holds together. Prints its seed;
# reproduce a failure with CAIRN_SOAK_SEED=<seed>.
soak:
	$(CARGO) test --test soak --release -- --ignored --nocapture

# Arbitrary argument vectors against the real binary. Runs a short pass as part
# of `make check`; this is the long one.
fuzz:
	CAIRN_FUZZ_ROUNDS=20000 $(CARGO) test --release --test fuzz_args -- --nocapture

# Line and region coverage, with a floor. Needs `cargo install cargo-llvm-cov`.
#
# The floor is not a target to creep towards; it is a ratchet. A change that
# drops below it has removed a test or added a surface nobody drove, and either
# is worth stopping for.
COVERAGE_FLOOR ?= 90
coverage:
	$(CARGO) llvm-cov --bins --tests --summary-only | tee /tmp/cairn-coverage.txt
	@awk '/^TOTAL/ { regions = $$4; lines = $$10; gsub("%","",regions); \
	  if (regions+0 < $(COVERAGE_FLOOR)) { \
	    printf("\ncoverage: %s of regions, %s of lines — below the floor of $(COVERAGE_FLOOR)%%\n", $$4, lines); \
	    printf("run `cargo llvm-cov --bins --tests --html` and look at what is red.\n"); \
	    exit 1 } \
	  printf("\ncoverage: %s of regions, %s of lines, floor $(COVERAGE_FLOOR)%%\n", $$4, lines) }' \
	  /tmp/cairn-coverage.txt

# Everything the project has. Run it before tagging, and after touching the
# lock, the write path, identifier allocation, or the merge driver.
#
# `make check` deliberately does not include these: both soak tests are
# `#[ignore]` and the fuzz pass is short, because a suite nobody will wait for
# is a suite nobody runs. The cost of that is that "check passes" proves less
# than it sounds like, so the difference has a name and a target rather than
# being folklore. Every stage prints the seed it used.
durability: check soak fuzz conformance
	@echo
	@echo "durability: suite, contention, 20000 fuzzed argument vectors,"
	@echo "            a second reader over every format that has existed."

demo: build
	python3 doc/demo.py --cairn $(CAIRN)

# Re-capture both recordings from real command output. `record` was declared
# phony with no rule, so it silently did nothing — while the CI job that checks
# the recordings are current told people to run it.
record: build
	python3 doc/demo.py --cairn $(CAIRN)
	python3 doc/samples.py --cairn $(CAIRN)

doc: info

info: doc/cairn.info
html: doc/cairn.html
pdf: doc/cairn.pdf

doc/cairn.info: doc/cairn.texi
	$(MAKEINFO) --output=$@ $<

doc/cairn.html: doc/cairn.texi
	$(MAKEINFO) --html --no-split --output=$@ $<

doc/cairn.pdf: doc/cairn.texi
	texi2pdf --output=$@ $<

# Regenerate the project's own roadmap and agent instructions.
roadmap: build
	$(CAIRN) render
	$(CAIRN) agent --write AGENTS.md

install: install-bin install-man install-info

install-bin: build
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 $(CAIRN) $(DESTDIR)$(BINDIR)/cairn

install-man: build
	install -d $(DESTDIR)$(MANDIR)
	$(CAIRN) man --dir $(DESTDIR)$(MANDIR)

install-info: doc/cairn.info
	install -d $(DESTDIR)$(INFODIR)
	install -m 644 doc/cairn.info $(DESTDIR)$(INFODIR)/cairn.info
	-install-info --dir-file=$(DESTDIR)$(INFODIR)/dir $(DESTDIR)$(INFODIR)/cairn.info

# A source tarball for packagers, and for anybody auditing what a release
# contains without trusting the forge to stay up.
#
# Built with `git archive` rather than by hand. That is not laziness: git
# archive is deterministic by construction — file order from the tree, mtimes
# from the commit, uid and gid zero — so the tarball CI publishes and the one
# you build here are the same bytes. Doing it with tar(1) instead means fighting
# the differences between GNU and BSD tar and touch, and losing quietly on
# somebody else's machine.
#
# The Info manual is not included: building it needs texinfo, which a packager
# has, and shipping a generated file would break the byte-for-byte property
# above. `make doc` produces it.
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
DIST    := cairn-$(VERSION)

dist:
	@rm -f $(DIST).tar.gz
	git archive --format=tar.gz -9 --prefix=$(DIST)/ -o $(DIST).tar.gz HEAD
	@echo "$(DIST).tar.gz"
	@shasum -a 256 $(DIST).tar.gz 2>/dev/null || sha256sum $(DIST).tar.gz

clean:
	$(CARGO) clean
	rm -f doc/cairn.info doc/cairn.html doc/cairn.pdf
	rm -rf cairn-*.tar.gz cairn-[0-9]*/
