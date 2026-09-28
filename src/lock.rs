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
// test-and-set on every supported platform and needs no dependency, and it
// excludes across containers, mounts and filesystems where an operating-system
// lock might not. Readers never take it: listing a backlog must not queue
// behind somebody's write.
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

/// How long a stale lock may refuse to be removed, in the way that on Windows
/// usually means somebody has it open for a moment, before that is reported
/// as the failure it then more likely is.
const UNREMOVABLE_AFTER: Duration = Duration::from_secs(10);

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
            Some(common) => Lock::acquire_within(
                common.join("cairn-claim.lock"),
                &|| common.join(BREAKER),
                HELD_TOO_LONG,
                WAIT_AT_MOST,
            )
            .map(Some),
            None => Ok(None),
        }
    }

    fn acquire_unchecked(cfg: &Config) -> Result<Lock> {
        Lock::acquire_within(
            Self::path(cfg),
            &|| Self::breaker(cfg),
            HELD_TOO_LONG,
            WAIT_AT_MOST,
        )
    }

    /// `breaker` says where the turn at breaking a stale lock is taken, and is
    /// only asked when there is a stale lock, because asking git is not free.
    fn acquire_within(
        path: PathBuf,
        breaker: &dyn Fn() -> PathBuf,
        held_too_long: Duration,
        at_most: Duration,
    ) -> Result<Lock> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }

        let started = std::time::Instant::now();
        // When the stale lock was first found impossible to remove for the
        // moment. See where it is set.
        let mut stuck: Option<std::time::Instant> = None;
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
                match Self::break_stale(&path, &breaker())? {
                    Breaking::Broke(broken) => {
                        stuck = None;
                        eprintln!(
                            "{} breaking a lock left behind {} seconds ago ({})",
                            style::yellow("warning:"),
                            broken.as_secs(),
                            path.display()
                        );
                        continue;
                    }
                    Breaking::Left => stuck = None,
                    // A file somebody has open for a moment on Windows looks
                    // exactly like one that can never be removed, so it is
                    // given a while before being called the second.
                    Breaking::InUse(e) => {
                        let since = *stuck.get_or_insert_with(std::time::Instant::now);
                        if since.elapsed() > UNREMOVABLE_AFTER {
                            return Err(e).with_context(|| {
                                format!("removing the stale lock {}", path.display())
                            });
                        }
                    }
                }
            } else if let Some(age) = age
                && age > held_too_long
            {
                // An age that cannot be read is a lock let go of between the
                // attempt and the look, so there is nobody to give up on.
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

    /// Where the turn at breaking the item directory's stale lock is taken.
    ///
    /// In a repository, in Git's common directory, where nothing shows in
    /// `git status` or is caught by `git add -A`, and which every worktree
    /// shares: breaking is rare enough that one turn for all of them costs
    /// nothing. Outside one, in a hidden directory beside the items, which the
    /// item scan does not enter and which no repository is there to see.
    fn breaker(cfg: &Config) -> PathBuf {
        match crate::worktree::common_dir(&cfg.root) {
            Some(common) => common.join(BREAKER),
            None => cfg.items_dir().join(".cairn").join(".lock"),
        }
    }

    /// Remove the stale lock at `path` if it is still stale once this waiter
    /// has its turn at breaking it.
    ///
    /// Breaking used to be `remove_file` and try again. Two waiters that both
    /// saw the lock stale could both remove it: the first then made its own
    /// lock and the second removed that one, so both held the lock and could
    /// allocate the same id. The look and the removal cannot be made one
    /// filesystem call, so breakers take turns instead: whoever holds the
    /// operating-system lock on `breaker` looks again and removes the lock
    /// only if it is still stale. Nothing else removes it meanwhile — a
    /// holder removes only its own, and the stale one's is gone — so what the
    /// breaker looks at is what it removes. The next breaker finds the first
    /// one's fresh lock, or none, and leaves it.
    ///
    /// Only breaking takes this lock. Holding the lock itself is still the
    /// `create_new` of `path`, which excludes a container from its host and
    /// one machine from another on a shared filesystem, where operating-system
    /// locks may be separate or missing. Those can then break a stale lock
    /// together as before, and so can an older cairn, which knows nothing of
    /// `breaker`. Where the turn cannot be taken at all — a read-only `.git`,
    /// or a network share without lock support — this breaks as before rather
    /// than never, and says so once.
    fn break_stale(path: &Path, breaker: &Path) -> Result<Breaking> {
        let turn = match Self::open_breaker(breaker) {
            Ok(file) => match file.try_lock() {
                Ok(()) => Some(file),
                Err(std::fs::TryLockError::WouldBlock) => return Ok(Breaking::Left),
                Err(std::fs::TryLockError::Error(e)) => {
                    without_a_turn(breaker, &e);
                    None
                }
            },
            Err(e) => {
                without_a_turn(breaker, &e);
                None
            }
        };

        let result = match Self::age(path) {
            Some(age) if age > STALE_AFTER => match std::fs::remove_file(path) {
                Ok(()) => Ok(Breaking::Broke(age)),
                // Gone already, which only a breaker without the turn does.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Breaking::Left),
                Err(e) if in_use(&e) => Ok(Breaking::InUse(e)),
                Err(e) => {
                    Err(e).with_context(|| format!("removing the stale lock {}", path.display()))
                }
            },
            _ => Ok(Breaking::Left),
        };
        if let Some(file) = turn {
            // Letting go at once rather than when the handle closes, which
            // Windows may take its time over. A failure here is ignored
            // because closing the handle, just after, lets go regardless.
            let _ = file.unlock();
        }
        result
    }

    /// Open the breaker's file to lock it: for writing, because Linux emulates
    /// an exclusive lock over NFS with one that needs a writable handle, but
    /// read-only where another user of a shared repository made the file and
    /// this one may not write it, since everywhere else a lock needs no more.
    fn open_breaker(breaker: &Path) -> std::io::Result<std::fs::File> {
        if let Some(dir) = breaker.parent() {
            std::fs::create_dir_all(dir)?;
        }
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(breaker)
        {
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                std::fs::File::open(breaker)
            }
            opened => opened,
        }
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

    /// A time in the future is treated as no time at all. Read as an age of
    /// nothing, it made a lock from a clock set ahead unbreakable for as long
    /// as the clock was out, where its mtime still says how old it is.
    fn recorded_age(path: &Path) -> Option<Duration> {
        let text = std::fs::read_to_string(path).ok()?;
        let since: u64 = text
            .lines()
            .find_map(|l| l.strip_prefix("since "))?
            .trim()
            .parse()
            .ok()?;
        now_seconds().checked_sub(since).map(Duration::from_secs)
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

/// The turn at breaking stale locks, in Git's common directory.
const BREAKER: &str = "cairn-break.lock";

/// What came of a turn at breaking a stale lock.
enum Breaking {
    /// It was removed, and was this old.
    Broke(Duration),
    /// It was not, because it had been broken already, or is not stale after
    /// all, or another waiter has the turn.
    Left,
    /// It could not be removed, perhaps only for the moment.
    InUse(std::io::Error),
}

/// Say, once in a process, that stale locks are being broken without taking
/// turns, and why. Not an error, because breaking without a turn is what
/// cairn always did, and refusing to would wedge the project instead.
fn without_a_turn(breaker: &Path, e: &std::io::Error) {
    static SAID: std::sync::Once = std::sync::Once::new();
    SAID.call_once(|| {
        eprintln!(
            "{} breaking a stale lock without taking turns: cannot lock {} ({e})",
            style::yellow("warning:"),
            breaker.display()
        );
    });
}

/// Whether removing a file failed only because another process has it open.
/// On Windows a file somebody has open without delete sharing — a virus
/// scanner or an indexer looking at it is enough — cannot be removed for a
/// moment, and one pending deletion refuses with access denied. Elsewhere
/// neither happens, and permission denied means what it says.
fn in_use(e: &std::io::Error) -> bool {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    cfg!(windows)
        && (e.kind() == std::io::ErrorKind::PermissionDenied
            || e.raw_os_error() == Some(ERROR_SHARING_VIOLATION))
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The breaker's file for a lock at `path`, placed as `Lock::breaker`
    /// places it beside the items.
    fn breaker(path: &Path) -> PathBuf {
        path.with_file_name(".cairn").join(".lock")
    }

    /// Take the lock at `path` as a writer does, waiting at most `at_most`.
    fn take(path: &Path, at_most: Duration) -> Result<Lock> {
        Lock::acquire_within(
            path.to_path_buf(),
            &|| breaker(path),
            Duration::from_secs(10),
            at_most,
        )
    }

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
        let lock = Lock::acquire_within(
            path.clone(),
            &|| breaker(&path),
            held_too_long,
            Duration::from_secs(60),
        );
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
        let err = take(&path, Duration::from_secs(60))
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
        let err = take(&path, Duration::from_secs(1))
            .err()
            .expect("it gave up");
        holders.join().expect("queue");
        assert!(err.to_string().contains("kept writing"), "{err}");
    }

    /// What a process that died holding the lock long ago leaves behind.
    const STALE: &str = "pid 999999\nsince 1000000000\n";

    /// However many waiters find the same stale lock at once, they hold the
    /// lock one at a time. Each counts itself in while it holds it, and two
    /// in at once is the bug: a late breaker deleting the lock an early one
    /// had just made.
    #[test]
    fn waiters_racing_to_break_one_stale_lock_hold_it_one_at_a_time() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        const WAITERS: usize = 16;
        for _ in 0..5 {
            let dir = tempfile::tempdir().expect("tempdir");
            let path = dir.path().join(".lock");
            std::fs::write(&path, STALE).expect("planting");
            let start = std::sync::Barrier::new(WAITERS);
            let holding = AtomicUsize::new(0);
            let most = AtomicUsize::new(0);
            std::thread::scope(|s| {
                for _ in 0..WAITERS {
                    s.spawn(|| {
                        start.wait();
                        let lock = take(&path, Duration::from_secs(60)).expect("every waiter");
                        most.fetch_max(
                            holding.fetch_add(1, Ordering::SeqCst) + 1,
                            Ordering::SeqCst,
                        );
                        std::thread::sleep(Duration::from_millis(10));
                        holding.fetch_sub(1, Ordering::SeqCst);
                        drop(lock);
                    });
                }
            });
            assert_eq!(
                most.load(Ordering::SeqCst),
                1,
                "two waiters held the lock at once"
            );
            assert!(!path.exists(), "the last holder let go");
        }
    }

    /// A lock taken a moment ago is waited on, never broken, and nobody even
    /// reaches for the turn at breaking one.
    #[test]
    fn a_fresh_lock_is_never_broken() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        plant(&path, 1, 0);
        let fresh = std::fs::read(&path).expect("reading");
        let err = take(&path, Duration::from_secs(1))
            .err()
            .expect("it waited");
        assert!(err.to_string().contains("kept writing"), "{err}");
        assert_eq!(std::fs::read(&path).expect("still there"), fresh);
        assert!(!breaker(&path).exists(), "breaking was never tried");
    }

    /// Two breakers that saw the same stale lock break it once: the second,
    /// taking its turn after the first has broken it and taken the lock,
    /// finds that fresh lock and leaves it.
    #[test]
    fn a_stale_lock_is_broken_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        std::fs::write(&path, STALE).expect("planting");

        let first = Lock::break_stale(&path, &breaker(&path)).expect("first breaker");
        assert!(
            matches!(first, Breaking::Broke(_)),
            "the first breaker broke it"
        );
        Lock::try_create(&path).expect("and took the lock");
        let taken = std::fs::read(&path).expect("reading");

        let second = Lock::break_stale(&path, &breaker(&path)).expect("second breaker");
        assert!(
            matches!(second, Breaking::Left),
            "the second broke the first one's lock"
        );
        assert_eq!(std::fs::read(&path).expect("still there"), taken);
    }

    /// While another waiter has its turn at breaking, a waiter that also saw
    /// the lock stale removes nothing, and waits rather than giving up on it.
    #[test]
    fn a_waiter_without_the_turn_removes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        std::fs::write(&path, STALE).expect("planting");
        let turn = Lock::open_breaker(&breaker(&path)).expect("opening the breaker");
        turn.try_lock().expect("another waiter's turn");

        let broke = Lock::break_stale(&path, &breaker(&path)).expect("no error");
        assert!(matches!(broke, Breaking::Left));
        let err = take(&path, Duration::from_secs(1))
            .err()
            .expect("it waited");
        assert!(err.to_string().contains("kept writing"), "{err}");
        assert_eq!(std::fs::read_to_string(&path).expect("still there"), STALE);

        drop(turn);
        take(&path, Duration::from_secs(5)).expect("broken once the turn is free");
    }

    /// A stale lock that cannot be removed says so. It used to be removed with
    /// the error discarded and tried again at once, for ever. A directory in
    /// the lock's place is one that cannot be removed as a file, and ageing a
    /// directory is only portable on Unix.
    #[cfg(unix)]
    #[test]
    fn a_stale_lock_that_cannot_be_removed_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        std::fs::create_dir(&path).expect("planting");
        let long_ago = SystemTime::now() - Duration::from_secs(3600);
        std::fs::File::open(&path)
            .and_then(|f| f.set_times(std::fs::FileTimes::new().set_modified(long_ago)))
            .expect("ageing the planted lock");
        let err = take(&path, Duration::from_secs(5))
            .err()
            .expect("it could not");
        assert!(
            format!("{err:#}").contains("removing the stale lock"),
            "{err:#}"
        );
    }

    /// Where no turn can be taken — here because the breaker's directory
    /// cannot be made, as under a read-only `.git` — a stale lock is still
    /// broken, without one, rather than wedging the project.
    #[test]
    fn a_stale_lock_is_broken_without_a_turn_where_none_can_be_taken() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        std::fs::write(&path, STALE).expect("planting");
        let in_the_way = dir.path().join("not-a-directory");
        std::fs::write(&in_the_way, "").expect("planting a file where a directory would go");
        let lock = Lock::acquire_within(
            path.clone(),
            &|| in_the_way.join(".lock"),
            Duration::from_secs(10),
            Duration::from_secs(5),
        )
        .expect("broken without a turn");
        assert!(
            std::fs::read_to_string(&path)
                .expect("taken")
                .contains(&format!("pid {}", std::process::id()))
        );
        drop(lock);
    }

    /// A lock whose recorded time is in the future, from a clock set ahead,
    /// is aged by its mtime instead of counting as brand new for ever.
    #[test]
    fn a_lock_from_the_future_is_aged_by_its_mtime() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(".lock");
        std::fs::write(&path, format!("pid 1\nsince {}\n", now_seconds() + 3600))
            .expect("planting");
        let long_ago = SystemTime::now() - Duration::from_secs(3600);
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_times(std::fs::FileTimes::new().set_modified(long_ago)))
            .expect("ageing the planted lock");
        assert!(Lock::age(&path).is_some_and(|age| age > STALE_AFTER));
        take(&path, Duration::from_secs(5)).expect("broken by its mtime");
    }
}
