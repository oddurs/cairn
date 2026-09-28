---
id: 163
uid: 42027f23-27d4-4453-9567-23fa387bd091
title: Two writers breaking the same stale lock must not both hold it
type: bug
status: done
milestone: v1.0
assignee: oddurs
depends_on:
- 161
created: 2026-09-28
updated: 2026-09-28
closed_at: 2026-09-28
priority: p2
effort: s
area: cli
---

## What happens

`Lock::acquire_within` breaks a lock older than five minutes by removing the
file and trying again. Two waiters can both read the stale lock's age. The
first removes it and creates its own. The second then removes the first's
fresh lock and creates another, so both believe they hold the lock and can
allocate the same id. The `let _ =` on that removal also discards its
error. Found reviewing 0161, and it predates that change.

## Approach

Break a stale lock by taking it over atomically, not by delete-then-create.
For example, rename it to a name unique to this waiter and check what was
renamed is the stale file just read, so only one waiter wins the takeover.
Or re-read the lock's contents immediately before removing it and remove it
only if they are unchanged, which narrows the window but does not close it.

## Acceptance criteria

- [x] A test with two waiters racing to break one stale lock ends with exactly one holder
- [x] A failure to remove a stale lock is reported, not discarded

## 2026-09-28

Design: breakers take turns. A waiter that finds the lock stale must first create <lock>.break with create_new; holding it, it looks at the lock again (the caller's look may be out of date) and takes it over by renaming it to a name of its own (<lock>.stale.<pid>.<nanos>.<n>), checking that what it moved has the bytes it just read and is still past STALE_AFTER, then deleting it and reporting any error but NotFound. While the turn is held nothing else removes the lock, so the file looked at is the file moved. If the check fails, the moved file is a live lock and is put back with hard_link, which fails rather than replace a lock taken meanwhile; that failure is an error. A turn older than 10s (BREAKER_GONE_AFTER) was abandoned by a breaker that died and is taken over by the same rename-and-verify, unprotected. Remaining window: only after a breaker has held its turn past 10s (died or was stopped) can two breakers run at once, and then a waiter creating the lock between one breaker's rename of a live lock and its link back holds it alongside that lock's holder (reported as an error by the breaker). Rename on Windows failing with access denied or a sharing violation is retried as contention. The warning is printed after breaking, so a blocked stderr cannot stall a breaker mid-break. Mixed versions: an older cairn breaking the same lock still races as before.

## 2026-09-28

Evidence, watched on macOS under load. waiters_racing_to_break_one_stale_lock_hold_it_one_at_a_time (16 threads, a planted 'pid 999999 since 1000000000' lock, 5 rounds, overlap counted with an AtomicUsize): with the old remove-and-retry patched back in it failed 10 of 10 runs ('two waiters held the lock at once'). Rename-and-verify without the turn failed 8 of 10: the link back hit 'File exists (os error 17)', which is that window reached in a thundering herd, so the turn is what closes it. The fix passed 20 of 20 runs of that test and then 30 of 30 runs of all seven lock tests. a_stale_lock_that_cannot_be_removed_is_reported (Unix only: a directory aged by set_times stands in for an unremovable lock, since ageing a directory is not portable to Windows) errors with 'removing the stale lock' at once; against the old code it never returned, spinning past 60s because the discarded error was retried with no pause. Also covered: a live lock moved by mistake is put back intact, and an abandoned turn is taken over. make durability passed.

## Result

A stale lock is broken by one waiter at a time: the waiter takes an operating-system lock (File::try_lock, MSRV 1.89) on cairn-break.lock in Git's common directory, or .cairn/.lock beside the items outside a repository, looks again, and removes the lock only if it is still stale. create_new on .lock remains the exclusion between writers, as before. Sixteen threads racing one stale lock hold it one at a time, where break-by-remove let two in every run. A failure to remove a stale lock is reported, on Windows after it persists for ten seconds. Where no turn can be taken, cairn says so once and breaks as before; breakers that do not share the turn (separate machines, a container and its host, older cairns) can still double-break.

## 2026-09-28

Design changed after review of PR #122. The turn-file design (<lock>.break plus rename-and-verify with a hard-link put-back) reproduced the race one layer down: (1) taking over an abandoned turn had no turn protecting it, so the rename/hard_link race recurred on the turn file under a herd; (2) Lock::drop removes whatever is at the path, so a holder whose lock was moved aside, or a live holder past 300 s, deletes the next holder's lock; (3) a slow breaker taken over after 10 s removed its successor's turn; (4) the 10 s wall-clock BREAKER_GONE_AFTER treated stalls and clock skew as death; (5) a turn with a future since never aged and wedged the project; (6) .lock.break and .lock.stale.* were not gitignored, and migrate --commit runs git add -A on the items dir; (7) on Windows a sharing violation on the post-rename remove was fatal; (8) hard_link is unsupported on exFAT and many network filesystems. Replaced with an OS lock: File::try_lock (MSRV raised to 1.89), polled, on a file that is never deleted: <git-dir>/cairn/items-<items path, percent-encoded>.lock in a repository (git dir found by walking up to .git, so every cairn reaches the same file with or without git on PATH), .lock.os beside the items outside one, <common>/cairn/claim.lock for claims. The kernel releases it when its holder dies. While holding it a writer still create_new's .lock (and cairn-claim.lock) so older cairns wait, marked 'lock os'. The holder removes a marked .lock at once (its maker is dead: nobody makes one without the OS lock) and breaks an unmarked one only by the old 300 s rule; only the OS-lock holder breaks anything, so new cairns never double-break. Drop removes .lock, then unlocks. Waiting keeps 0161's limits; a waiter without the OS lock gives up only when the same over-limit .lock is still there unchanged after 1 s, so a herd arriving at a dead marked lock does not give up on it while the holder is removing it. A filesystem where locking is Unsupported falls back to the old unmarked scheme.

## 2026-09-28

Evidence for the OS-lock design, watched on macOS under load. Race test (16 threads, rounds alternating a planted marked and an unmarked stale .lock, overlap counted with an AtomicUsize): 20 of 20 runs of all lock tests passed, then 5 of 5 more after adding the last test. Against origin/main's lock through a shim that ignores the OS-lock path, the same test failed the overlap assertion 10 of 10 ('two waiters held the lock at once'). Under the new design exclusion comes from the kernel, so the race test shows it holds rather than probing a narrow window. New tests: a marked .lock nobody holds is taken at once; an unmarked fresh .lock is waited on (gives up with 'kept writing' and is left alone) and an unmarked stale one is broken by age; a child process (the test binary re-invoked) takes the lock, keeps a second writer out, then exits via process::exit without Drop, leaving its marked .lock, and the next writer takes it at once; the OS lock's file lands in .git/cairn/ and through a linked worktree's .git file; an abandoned lock that cannot be removed (a directory, Unix only) is reported as 'removing the abandoned lock'. Two existing tests assumed no lock file survives a command and now exempt .lock.os explicitly (soak, split). cargo +1.89 check --all-targets passes; clippy -D warnings, make durability pass.

## 2026-09-28

Design narrowed after a second review (of the OS-lock version on PR #122). Making the OS lock the exclusion cost more than it saved: (1) a container and its host, WSL and Windows, or NFS without lock sharing can each lock a different file, or locks not shared between them, and each then deleted the other's live marked .lock, where create_new on .lock used to exclude them; (2) a read-only or foreign .git made every write fail; (3) a persistent lock file opened read-write kept its first creator's umask and locked other users of a group-shared repository out; (4) only Unsupported fell back, so ENOLCK (NFS without lockd) and some Windows errors were fatal; (5) held_on trusted dangling gitdir paths, created foreign trees, and swallowed canonicalize and read errors; (6) it warned before removing and on every poll; (7) .lock.os was not in existing projects' items/.gitignore; (8) percent-encoded names could pass NAME_MAX; (9) it duplicated cmd/git.rs's gitdir parsing. Now: create_new on .lock is again the one exclusion, exactly as on main; no marker, no git-dir placement, no .lock.os. The OS lock only serialises breaking: a waiter that sees .lock past STALE_AFTER takes File::try_lock on a persistent <items>/.cairn/.lock (claims: cairn-claim.breaker beside cairn-claim.lock in the git common dir), opened read-only when it exists so a group-shared repository keeps working, re-reads .lock, and removes it only if it has the same bytes and is still past STALE_AFTER. Then it releases and loops to create_new. A second breaker re-reads and finds the first one's fresh lock, so leaves it. WouldBlock means another breaker's turn: remove nothing and wait. Any other open or try_lock error falls back to breaking by age for that attempt, never fatal. Removal errors other than NotFound (and the Windows sharing-violation retry) are reported. The warning comes once, after removing. The existing items/.gitignore pattern .lock already matches .cairn/.lock (checked with git check-ignore), and the item scan skips dot directories (store.rs collect checks the name before recursing). MSRV stays 1.89 for File::try_lock. Residual race, now in the manual: breakers on separate machines, a container and its host, older cairns, or a filesystem that cannot lock do not share the turn and can still double-break, as before.

## 2026-09-28

Evidence for the narrowed design, watched on macOS under load. Race test (16 threads, a planted 'pid 999999 since 1000000000' .lock, 5 rounds, overlap counted with an AtomicUsize): all 8 lock tests passed 20 of 20 runs. Against origin/main's lock through a shim, the same race test failed the overlap assertion 10 of 10 ('two waiters held the lock at once'). What it establishes: within one machine and one kernel, serialised breaking plus the re-read gives one holder at a time under a herd, where break-by-remove did not. It says nothing about the fallback, which could not be provoked portably. Deterministic tests: a fresh .lock is waited on and never broken, and the breaker file is never created; a stale lock is broken once, since a second breaker holding the same stale bytes finds the first one's fresh lock and leaves it; a waiter that cannot take the turn (held by another handle) removes nothing and waits ('kept writing'), and breaks once the turn is free; an unremovable stale lock (a directory, Unix only) is reported as 'removing the stale lock'. Gate: fmt, clippy -D warnings, cargo +1.89 check --all-targets and make durability passed; make check was rerun after a final comment-only edit.

## 2026-09-28

Third review (of a03c9ea) fixed on the same design, rebased onto 096d8a4. (1) The breaker file was opened read-only when it existed, which likely defeats try_lock on Linux NFS, where flock is emulated with fcntl and an exclusive lock needs a writable handle; it is now opened read-write, falling back to read-only only on PermissionDenied (the group-shared case). (2) <items>/.cairn/.lock is untracked in a project with no items/.gitignore, cairn's own included, so migrate's dirty check would refuse and git add -A would commit it; in a repository the turn is now <common>/cairn-break.lock, found through worktree::common_dir only when a stale lock is seen, and shared by every worktree's item lock and the claim lock (cairn-claim.breaker folded into it). Outside a repository it stays <items>/.cairn/.lock. (3) On Windows every PermissionDenied from remove_file was treated as transient, so a truly unremovable stale lock ended as 'kept writing' after 120 s; the same stale lock staying unremovable for 10 s (UNREMOVABLE_AFTER) is now reported as the removal error. (5) A since in the future read as age 0 for ever; it is now treated as unreadable, so the mtime fallback ages it. (6) The seen-bytes comparison is gone: re-checking staleness under the turn is enough, and age/recorded_age are back to their original shapes. (7) The first no-turn fallback in a process warns once on stderr with the reason; unlock() errors are ignored with a comment, since closing the handle releases the lock. (8) The Cargo.toml MSRV comment now says try_lock serialises breaking and create_new is still the write lock. The manual's residual-cases sentence now mentions a breaker stopped while holding its turn.

## 2026-09-28

Evidence after the third review, watched on macOS under load. All 10 lock unit tests passed 20 of 20 runs; the race test against origin/main's lock through a shim failed 10 of 10 ('two waiters held the lock at once'). New tests: with the breaker's parent a regular file (create_dir_all fails on every OS), a stale lock is still broken without a turn, and the once-per-process warning printed ('breaking a stale lock without taking turns: cannot lock ... (File exists)'); a lock whose since is an hour ahead with an mtime an hour old is aged by the mtime and broken; an integration test in a real repository with items/.gitignore removed breaks a stale lock and leaves git status free of .cairn, .lock or break files, with .git/cairn-break.lock present. Not tested: the read-write-then-read-only open on NFS, and the Windows UNREMOVABLE_AFTER bound, since in_use only applies on Windows. Gate: fmt --check, clippy -D warnings, cargo +1.89 check --all-targets, make durability (which runs make check) passed.
