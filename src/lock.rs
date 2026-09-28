// cairn — the repository lock.
//
// Copyright (c) 2026 Oddur Sigurdsson. MIT licensed; see LICENSE.
//
// Allocating an id means reading every id in use and adding one, which is only
// correct if nothing else is doing the same thing. That was an acceptable
// assumption when the writer was a person at a terminal. It stopped being one
// when two agents working the same backlog became the advertised use case.
//
// The mechanism is a lock file created with `create_new`, which is an atomic
// test-and-set on every supported platform and needs no dependency. Readers
// never take it: listing a backlog must not queue behind somebody's write.
use crate::config::Config;
use crate::style;
use anyhow::{Context, Result, bail};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long one holder may keep the lock before a waiter gives up and says
/// what holds it. It is the holder's time, from the `since` it recorded, and
/// not the waiter's: forty agents writing at once each hold the lock briefly,
/// and on a loaded machine the last in line waits longer than any of them
/// holds it. That queue is moving. A holder ten seconds into a write is not.
const HELD_TOO_LONG: Duration = Duration::from_secs(10);

/// How long to wait in all, however the lock is changing hands. Taking it is a
/// race rather than a queue, so a waiter can lose it to later arrivals for as
/// long as they keep coming; this makes that an error rather than a silent
/// hang, generously enough that forty writers on a busy machine never meet it.
const WAIT_AT_MOST: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(25);

/// A lock older than this is assumed to belong to a process that died. Long
/// enough that a slow import is never mistaken for a corpse.
const STALE_AFTER: Duration = Duration::from_secs(300);

/// Held for the duration of a write, released by dropping it.
pub struct Lock {
    path: PathBuf,
}

impl Lock {
    /// Take the lock, waiting for another writer to finish if necessary.
    ///
    /// Also the one place a project behind the current format is turned away.
    /// Every command that changes the backlog takes the lock and no read ever
    /// does, so this is the whole boundary — and it means the rule can be
    /// stated in one sentence rather than repeated at twenty call sites:
    ///
    /// > A format bump may cost somebody a command. It must never cost them
    /// > their data, and it must never cost them the ability to look.
    pub fn acquire(cfg: &Config) -> Result<Lock> {
        // A person at a terminal is asked, rather than told to go and run
        // something. On yes the project is migrated, and the command starts
        // over, because everything it has read so far was read at the old
        // format. Agents, pipes and scripts are never asked.
        if cfg.format() < crate::config::CURRENT_FORMAT
            && crate::cmd::migrate::may_offer(
                std::io::stdin().is_terminal() && std::io::stderr().is_terminal(),
                crate::store::acting_agent().is_some(),
            )
            && crate::cmd::migrate::offer(cfg)?
        {
            let status = std::process::Command::new(std::env::current_exe()?)
                .args(std::env::args_os().skip(1))
                .status()?;
            std::process::exit(status.code().unwrap_or(1));
        }
        if cfg.format() < crate::config::CURRENT_FORMAT {
            // What migrating costs, said where somebody is stopped by it rather
            // than only in the manual. The recurring one-line notice stays short
            // on purpose — it prints on every command — so this is the place the
            // detail belongs: it is reached exactly once, by somebody who now
            // needs it.
            anyhow::bail!(
                "this project is format {}, and writing needs format {}\n\
                 run `cairn migrate` (`--dry-run` first to see what it would change)\n\
                 \n\
                 format 5 gives every item a readable number and a `uid` tag. From\n\
                 format 4 it restores the numbers items had, numbers the rest in\n\
                 creation order, rewrites id references and renames files; {} is\n\
                 updated last. Commit the project first, migrate once and share\n\
                 that commit. Do not migrate divergent clones independently.\n\
                 \n\
                 reading works meanwhile: list, show, next, search, board, roadmap, log, export",
                cfg.format(),
                crate::config::CURRENT_FORMAT,
                crate::config::CONFIG_FILE
            );
        }
        Lock::acquire_unchecked(cfg)
    }

    /// Take the lock without the format check, for `cairn migrate` itself.
    pub fn acquire_for_migration(cfg: &Config) -> Result<Lock> {
        Lock::acquire_unchecked(cfg)
    }

    /// Take the lock every worktree of the repository shares, for as long as
    /// a claim reads what the others hold and writes its own.
    ///
    /// The item directory's lock cannot do this: each worktree has its own
    /// directory, so two agents claiming at the same moment each read the
    /// other's worktree before either had written, and both took the same
    /// item. Git's common directory is the one place all of them share.
    /// Outside a repository there is nothing else to exclude, and `None` is
    /// returned.
    pub fn acquire_across_worktrees(cfg: &Config) -> Result<Option<Lock>> {
        match crate::worktree::common_dir(&cfg.root) {
            Some(common) => Lock::acquire_at(common.join("cairn-claim.lock")).map(Some),
            None => Ok(None),
        }
    }

    fn acquire_unchecked(cfg: &Config) -> Result<Lock> {
        Lock::acquire_at(Self::path(cfg))
    }

    fn acquire_at(path: PathBuf) -> Result<Lock> {
        Lock::acquire_within(path, HELD_TOO_LONG, WAIT_AT_MOST)
    }

    fn acquire_within(path: PathBuf, held_too_long: Duration, at_most: Duration) -> Result<Lock> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }

        let started = std::time::Instant::now();
        loop {
            match Self::try_create(&path) {
                Ok(()) => return Ok(Lock { path }),
                Err(e) if contended(&e) => {}
                Err(e) => {
                    return Err(e).with_context(|| format!("creating {}", path.display()));
                }
            }

            let age = Self::age(&path);
            if let Some(age) = age
                && age > STALE_AFTER
            {
                eprintln!(
                    "{} breaking a lock left behind {} seconds ago ({})",
                    style::yellow("warning:"),
                    age.as_secs(),
                    path.display()
                );
                let _ = std::fs::remove_file(&path);
                continue;
            }

            // An age that cannot be read is a lock let go of between the
            // attempt and the look, so there is nobody to give up on.
            if let Some(age) = age
                && age > held_too_long
            {
                bail!(
                    "another cairn process is writing to this project\n\
                     {} has been held for {}s\n\
                     if nothing else is running, delete that file",
                    path.display(),
                    age.as_secs()
                );
            }
            let waited = started.elapsed();
            if waited > at_most {
                bail!(
                    "other cairn processes kept writing to this project\n\
                     waited {}s for {} while it changed hands; try again when they are done",
                    waited.as_secs(),
                    path.display()
                );
            }
            std::thread::sleep(POLL);
        }
    }

    /// Inside the item directory, where dotfiles are already ignored by the
    /// item scan and by the `.gitignore` that `cairn init` writes.
    ///
    pub fn path(cfg: &Config) -> PathBuf {
        cfg.items_dir().join(".lock")
    }

    /// `create_new` fails if the file exists, and does so atomically — which is
    /// the whole mechanism.
    fn try_create(path: &Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        // For a human reading it after something went wrong, and for `age` to
        // prefer over the file's mtime. If it fails — a full disk is the way —
        // `age` falls back, so a half-written lock is still breakable.
        let _ = writeln!(file, "pid {}\nsince {}", std::process::id(), now_seconds());
        Ok(())
    }

    /// How long ago the lock was taken.
    ///
    /// The recorded timestamp is preferred, because some filesystems keep mtime
    /// at a coarse resolution. But a lock file with no readable timestamp is
    /// exactly what a process that died *while creating it* leaves behind — a
    /// full disk between `create_new` and the write — and that lock used to be
    /// unbreakable, so the project it guarded was wedged until somebody deleted
    /// the file by hand. A coarse answer is worth having; no answer is not.
    fn age(path: &Path) -> Option<Duration> {
        Self::recorded_age(path).or_else(|| Self::mtime_age(path))
    }

    fn recorded_age(path: &Path) -> Option<Duration> {
        let text = std::fs::read_to_string(path).ok()?;
        let since: u64 = text
            .lines()
            .find_map(|l| l.strip_prefix("since "))?
            .trim()
            .parse()
            .ok()?;
        Some(Duration::from_secs(now_seconds().saturating_sub(since)))
    }

    fn mtime_age(path: &Path) -> Option<Duration> {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()?
            .elapsed()
            .ok()
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Whether a failed creation means somebody else holds the lock, rather than
/// something being genuinely wrong.
///
/// `AlreadyExists` is the ordinary case. `PermissionDenied` is the Windows one:
/// a deleted file stays in a "pending delete" state until the last handle to it
/// closes, and opens during that window fail with access denied rather than
/// with anything resembling "it exists". Treating that as fatal made a writer
/// give up while the previous holder was still letting go — which is precisely
/// what forty concurrent writers on Windows produce, and what a Linux-only test
/// run never shows.
fn contended(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::AlreadyExists | std::io::ErrorKind::PermissionDenied
    )
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lock as a holder that took it `ago` leaves it.
    fn plant(path: &Path, pid: u32, ago: u64) {
        let since = now_seconds() - ago;
        std::fs::write(path, format!("pid {pid}\nsince {since}\n")).expect("planting");
    }

    /// Hand the lock from holder to holder every `every`, then let it go.
    fn queue(path: &Path, holders: u32, every: Duration) -> std::thread::JoinHandle<()> {
        let path = path.to_path_buf();
        plant(&path, 1, 0);
        std::thread::spawn(move || {
            for pid in 2..=holders {
                std::thread::sleep(every);
                plant(&path, pid, 0);
            }
            std::thread::sleep(every);
            std::fs::remove_file(&path).expect("the last holder lets go");
        })
    }

    /// The whole wait outlasts the limit on any one holder, so a waiter timed
    /// from its own arrival — as it was — gives up on a queue that is moving.
    #[test]
    fn a_waiter_outlasts_a_queue_that_keeps_moving() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        let held_too_long = Duration::from_secs(3);
        let holders = queue(&path, 6, Duration::from_secs(1));
        let started = std::time::Instant::now();
        let lock = Lock::acquire_within(path, held_too_long, Duration::from_secs(60));
        holders.join().expect("queue");
        assert!(lock.is_ok(), "gave up on a moving queue: {:?}", lock.err());
        assert!(
            started.elapsed() > held_too_long,
            "the queue outlasted the limit, so this tested something"
        );
    }

    /// Measured from when the holder took it, not from when the waiter came:
    /// arriving late to a long write does not buy it another ten seconds.
    #[test]
    fn a_holder_that_keeps_the_lock_too_long_is_given_up_on() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        plant(&path, 1, 20);
        let started = std::time::Instant::now();
        let err = Lock::acquire_within(path, Duration::from_secs(10), Duration::from_secs(60))
            .err()
            .expect("it gave up");
        assert!(err.to_string().contains("has been held for 20s"), "{err}");
        assert!(started.elapsed() < Duration::from_secs(5), "at once");
    }

    #[test]
    fn a_waiter_gives_up_on_a_queue_that_never_ends() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        let holders = queue(&path, 4, Duration::from_millis(500));
        let err = Lock::acquire_within(path, Duration::from_secs(10), Duration::from_secs(1))
            .err()
            .expect("it gave up");
        holders.join().expect("queue");
        assert!(err.to_string().contains("kept writing"), "{err}");
    }
}
