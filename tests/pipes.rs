//! A reader that goes away before cairn has finished writing — `cairn list |
//! head -1` — is the ordinary end of a pipeline, not a crash: no panic, and
//! the status SIGPIPE gives, since the command may have stopped partway.
mod support;
use std::io::Read;
use std::process::{Command, Stdio};
use support::*;

/// Run cairn with its standard output already closed at the far end, as it is
/// once `head` has read what it wanted. Returns the exit code and stderr.
fn with_the_reader_gone(p: &Project, args: &[&str]) -> (i32, String) {
    let mut child = Command::new(bin())
        .args(args)
        .current_dir(p.root())
        .env("NO_COLOR", "1")
        .env("CAIRN_USER", "tester")
        .env("PATH", path_with_binary())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    // Closed before cairn has written a byte, so its first write meets it.
    drop(child.stdout.take());
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    let status = child.wait().expect("wait");
    (status.code().unwrap_or(-1), stderr)
}

#[test]
fn a_reader_that_stops_reading_is_not_a_crash() {
    let p = Project::new();
    for n in 1..=5 {
        p.add(&format!("Item {n}"), &[]);
    }
    for args in [
        vec!["list"],
        vec!["show", "1"],
        vec!["prompt", "1"],
        vec!["list", "--json"],
    ] {
        let (code, stderr) = with_the_reader_gone(&p, &args);
        assert_missing(&stderr, "panicked", &format!("{args:?}"));
        assert_eq!(code, 141, "{args:?}: as SIGPIPE reports it: {stderr}");
    }
}

/// A writer that loses its reader mid-command has already written, and must
/// not leave the project locked behind it.
#[test]
fn a_writer_that_loses_its_reader_still_lets_go_of_the_lock() {
    let p = Project::new();
    p.add("Take me", &[]);
    let (code, stderr) = with_the_reader_gone(&p, &["claim", "1"]);
    assert_missing(&stderr, "panicked", "");
    assert_eq!(code, 141, "{stderr}");
    assert_eq!(
        p.json(&["show", "1", "--json"])["assignee"],
        "tester",
        "the write landed"
    );
    p.expect(&["note", "1", "The next writer gets in."]);
}

/// Help is written before any command runs, and is covered too.
#[test]
fn help_to_a_reader_that_has_gone_is_quiet() {
    let p = Project::new();
    let (_, stderr) = with_the_reader_gone(&p, &["--help"]);
    assert_missing(&stderr, "panicked", "");
}
