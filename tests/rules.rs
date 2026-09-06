//! The rules from CONTRIBUTING, enforced.
//!
//! These tests read the repository rather than run the program. That is
//! deliberate: each one guards a decision that is invisible in the code — an
//! absence, or an agreement between files — and so cannot be guarded from
//! inside the program itself. Every one of them exists because the decision it
//! protects is the kind somebody would reverse without knowing it was a
//! decision.

use std::fs;
use std::path::PathBuf;

/// A repository file, with line endings normalised.
///
/// A Windows checkout may hold every one of these files with CRLF endings —
/// which is a supported thing for a checkout to do, and is how this function
/// came to exist: the enum parser below looked for "\n}\n" and found nothing.
fn repo(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    text.replace("\r\n", "\n")
}

/// Text between two markers, with every run of whitespace collapsed to one
/// space. Markers differ per file — Markdown comments, Texinfo comments — but
/// the words between them must not.
fn between(text: &str, begin: &str, end: &str, what: &str) -> String {
    let start = text
        .find(begin)
        .unwrap_or_else(|| panic!("{what} has no `{begin}` marker"))
        + begin.len();
    let rest = &text[start..];
    let stop = rest
        .find(end)
        .unwrap_or_else(|| panic!("{what} has no `{end}` marker"));
    rest[..stop]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The promise is made in three places and must be the same promise.
///
/// It is written without markup of any kind so that this comparison can be
/// exact — which is also why it is short enough to read, and those two
/// properties are not a coincidence.
#[test]
fn the_promise_is_stated_identically_everywhere() {
    let canonical = between(
        &repo("PROMISE.md"),
        "<!-- promise:begin -->",
        "<!-- promise:end -->",
        "PROMISE.md",
    );

    assert!(
        canonical.contains("will never grow accounts, authentication, or remotes"),
        "the promise no longer names the three things cairn will not grow"
    );

    for (file, begin, end) in [
        (
            "README.md",
            "<!-- promise:begin -->",
            "<!-- promise:end -->",
        ),
        ("doc/cairn.texi", "@c promise:begin", "@c promise:end"),
    ] {
        let copy = between(&repo(file), begin, end, file);
        assert_eq!(
            canonical, copy,
            "the promise in {file} has drifted from PROMISE.md.\n\
             It is reproduced word for word on purpose: a promise that says\n\
             something slightly different in each place it appears is not one."
        );
    }
}

/// A program that links cairn inherits the GPL; a program that reads cairn's
/// documented format does not. The absence of a library is what makes the
/// specification, rather than this code, the thing to build against.
#[test]
fn there_is_no_library_target() {
    let manifest = repo("Cargo.toml");
    let declared = manifest
        .lines()
        .map(str::trim)
        .any(|l| l == "[lib]" || l.starts_with("[lib."));

    assert!(
        !declared,
        "Cargo.toml declares a [lib] target.\n\
         This is deliberate and documented in CONTRIBUTING.md: linking cairn\n\
         makes the caller a derivative work, reading its file format does not.\n\
         A second program that wants items should read spec/README.md, or run\n\
         the binary and parse --json."
    );
}

/// The variants of `enum Command`, in source order, with the `hide` attribute
/// that precedes each.
fn commands() -> Vec<(String, bool)> {
    commands_in(&repo("src/main.rs"))
}

/// Parses arbitrary text rather than trusting `repo` to have normalised it —
/// the first version of this trusted its caller, and the caller on Windows was
/// a CRLF checkout.
fn commands_in(src: &str) -> Vec<(String, bool)> {
    let src = src.replace("\r\n", "\n");
    let src = src.as_str();
    let start = src.find("enum Command {").expect("enum Command");
    let body = &src[start..];
    let end = body.find("\n}\n").expect("end of enum Command");

    let mut out = Vec::new();
    let mut hidden = false;
    for line in body[..end].lines() {
        let line = line.trim();
        if line.contains("hide = true") {
            hidden = true;
            continue;
        }
        // A variant looks like `Name(cmd::x::Args),` at one level of indent.
        if let Some(paren) = line.find('(')
            && line.ends_with("),")
            && line[..paren].chars().all(|c| c.is_alphanumeric())
            && line.starts_with(|c: char| c.is_ascii_uppercase())
        {
            out.push((line[..paren].to_string(), hidden));
            hidden = false;
        }
    }
    assert!(out.len() > 20, "failed to parse the command enum");
    out
}

/// The helpers above must work against a checkout with CRLF endings, because
/// that is a thing a Windows checkout legitimately is.
///
/// This test exists because it was not here: `commands` looked for a literal
/// "\n}\n" and found nothing on Windows, so two rules passed vacuously for as
/// long as it took CI to run. A rule that cannot fail is worse than no rule,
/// and a rule that stops being able to fail on one platform is the same thing
/// wearing a disguise.
#[test]
fn the_parsers_survive_a_crlf_checkout() {
    let crlf = repo("src/main.rs").replace('\n', "\r\n");
    assert!(
        commands_in(&crlf).len() > 20,
        "the command enum is unparseable when the checkout uses CRLF endings"
    );

    let crlf = repo("PROMISE.md").replace('\n', "\r\n");
    let promise = between(
        &crlf,
        "<!-- promise:begin -->",
        "<!-- promise:end -->",
        "PROMISE.md",
    );
    assert!(
        promise.contains("accounts, authentication, or remotes"),
        "the promise is unreadable when the checkout uses CRLF endings"
    );
}

/// cairn will never grow accounts, authentication, or remotes.
///
/// The failure mode this guards is gradual: no single command called `login`
/// gets added on purpose. What happens is that a command needing a credential
/// appears, then one needing somewhere to send it, and by the time anybody
/// names the pattern the boundary has already moved. So the names are refused
/// individually, and the reason travels with the refusal.
#[test]
fn no_command_reaches_outside_the_repository() {
    // `import` and `export` are absent from this list on purpose: they move a
    // file, and the file gets there however files get anywhere. It is the
    // network, and the identity a network needs, that is out of bounds.
    const FORBIDDEN: &[&str] = &[
        "Login",
        "Logout",
        "Signin",
        "Signup",
        "Auth",
        "Authenticate",
        "Account",
        "User",
        "Token",
        "Remote",
        "Push",
        "Pull",
        "Fetch",
        "Clone",
        "Sync",
        "Serve",
        "Daemon",
        "Watch",
        "Notify",
        "Subscribe",
    ];

    for (name, _) in commands() {
        assert!(
            !FORBIDDEN.contains(&name.as_str()),
            "`cairn {}` would break the promise in PROMISE.md.\n\
             cairn does everything a single repository can do and nothing\n\
             beyond it, and a command that acquires a session, reaches a\n\
             server, or keeps running is outside that. If this capability is\n\
             genuinely needed, it belongs in a separate program that reads the\n\
             file format.",
            name.to_lowercase()
        );
    }
}

/// A command that exists to be tested rather than used is hidden from `--help`.
///
/// `migrate` is the interesting case. It is hidden today because format 1 is
/// the only format there has ever been, so nobody can need it; the day that
/// stops being true it becomes a command people run, and this test says so.
#[test]
fn commands_that_exist_only_to_be_tested_are_hidden() {
    let hidden: Vec<_> = commands()
        .into_iter()
        .filter(|(_, h)| *h)
        .map(|(n, _)| n)
        .collect();

    for expected in ["MergeDriver", "Migrate"] {
        assert!(
            hidden.contains(&expected.to_string()),
            "`{expected}` should be hidden from --help: see the second rule in \
             CONTRIBUTING.md"
        );
    }

    let config = repo("src/config.rs");
    let current: u32 = config
        .lines()
        .find_map(|l| l.trim().strip_prefix("pub const CURRENT_FORMAT: u32 = "))
        .and_then(|v| v.trim_end_matches(';').parse().ok())
        .expect("CURRENT_FORMAT in src/config.rs");

    assert_eq!(
        current, 1,
        "CURRENT_FORMAT is now {current}, so a project can genuinely be behind \
         and `cairn migrate` is a command people need to find. Remove the \
         `#[command(hide = true)]` from Migrate in src/main.rs, and this \
         assertion with it."
    );
}

/// The specification is normative and standalone, so it must not tell a reader
/// to go and look at the implementation to find out what something means.
#[test]
fn the_specification_does_not_defer_to_the_implementation() {
    let spec = repo("spec/README.md");
    for (n, line) in spec.lines().enumerate() {
        let lower = line.to_lowercase();
        let defers = lower.contains("see the source")
            || lower.contains("whatever cairn does")
            || lower.contains("refer to the implementation");
        assert!(
            !defers,
            "spec/README.md:{}: the specification defers to the implementation.\n\
             It has to stand alone: somebody must be able to write a reader \
             from it in ten years, with cairn unavailable.",
            n + 1
        );
    }
}
