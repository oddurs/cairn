// cairn — the repository lock.
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
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long to keep trying before giving up and telling the user what holds it.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(10);
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
    pub fn acquire(cfg: &Config) -> Result<Lock> {
        let path = Self::path(cfg);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }

        let deadline = std::time::Instant::now() + ACQUIRE_TIMEOUT;
        loop {
            match Self::try_create(&path) {
                Ok(()) => return Ok(Lock { path }),
                Err(e) if contended(&e) => {}
                Err(e) => {
                    return Err(e).with_context(|| format!("creating {}", path.display()));
                }
            }

            if let Some(age) = Self::age(&path)
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

            if std::time::Instant::now() >= deadline {
                bail!(
                    "another cairn process is writing to this project\n\
                     waited {}s for {}\n\
                     if nothing else is running, delete that file",
                    ACQUIRE_TIMEOUT.as_secs(),
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
        // Contents are for a human reading it after something went wrong.
        let _ = writeln!(file, "pid {}\nsince {}", std::process::id(), now_seconds());
        Ok(())
    }

    /// How long ago the lock was taken, by its recorded timestamp rather than
    /// the file's mtime, which some filesystems keep at a coarse resolution.
    fn age(path: &Path) -> Option<Duration> {
        let text = std::fs::read_to_string(path).ok()?;
        let since: u64 = text
            .lines()
            .find_map(|l| l.strip_prefix("since "))?
            .trim()
            .parse()
            .ok()?;
        Some(Duration::from_secs(now_seconds().saturating_sub(since)))
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
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
