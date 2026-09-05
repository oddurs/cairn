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

.PHONY: all build check test soak doc info html pdf demo install install-bin \
        install-man install-info clean roadmap

all: build doc

build:
	$(CARGO) build --release

# Everything CI runs.
check: test
	$(CARGO) fmt --check
	$(CARGO) clippy --all-targets -- -D warnings
	$(CAIRN) check --render --strict

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

demo: build
	python3 doc/demo.py --cairn $(CAIRN)

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

clean:
	$(CARGO) clean
	rm -f doc/cairn.info doc/cairn.html doc/cairn.pdf
