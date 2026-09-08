// cairn — laws the filter grammar has to obey.
//
// Copyright (C) 2026 Oddur Sigurdsson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License as published by the Free Software
// Foundation, either version 3 of the License, or (at your option) any later
// version.  See COPYING for details.
//
// The parser already has property tests: it may reject anything, and it may not
// panic. Those say nothing about what an accepted filter *means*.
//
// These state the laws instead. Negation partitions the backlog, alternation is
// union, and a comma is intersection. Each is checked against a generated
// backlog rather than a handful of items somebody chose, because the interesting
// cases — an unset field, a field only some items have — are the ones nobody
// thinks to write down.
mod support;
use support::*;

use std::collections::BTreeSet;

const STATUSES: &[&str] = &["backlog", "planned", "doing", "blocked", "done", "dropped"];
const TYPES: &[&str] = &["feature", "bug", "chore", "docs"];
const PRIORITIES: &[&str] = &["p0", "p1", "p2"];
const LABELS: &[&str] = &["auth", "ui", "perf"];
fn backlog(seed: u64, count: usize) -> Project {
    let p = Project::with_init(&["init", "--bare", "--name", "Laws"]);

    let mut rng = Rng::new(seed);
    // A milestone is an item in format 2, so it has to exist before anything
    // can be scheduled against it.
    let m = p
        .expect(&["new", "v0.1", "-t", "milestone", "-q"])
        .trimmed();
    p.expect(&["set", &m, "key=v0.1"]);

    for n in 0..count {
        let out = p.expect(&["new", &format!("Item {n}"), "-q"]);
        let id = out.trimmed().to_string();
        p.expect(&[
            "set",
            &id,
            &format!("status={}", rng.choose(STATUSES)),
            &format!("type={}", rng.choose(TYPES)),
            "-q",
        ]);
        // Deliberately left unset on roughly a third of items.
        if rng.below(3) != 0 {
            p.expect(&[
                "set",
                &id,
                &format!("priority={}", rng.choose(PRIORITIES)),
                "-q",
            ]);
        }
        if rng.below(3) != 0 {
            p.expect(&["set", &id, "milestone=v0.1", "-q"]);
        }
        if rng.below(2) == 0 {
            p.expect(&["set", &id, &format!("labels+={}", rng.choose(LABELS)), "-q"]);
        }
    }
    p
}

/// `key=value` and `key!=value` divide the backlog in two, with nothing in both
/// and nothing in neither.
///
/// The case this exists for is the unset field. An item with no priority has to
/// land on exactly one side of `priority=p0`, and it is not obvious in advance
/// which — only that it cannot be on both or on neither.
#[test]
fn negation_partitions_the_backlog() {
    let p = backlog(0xF11, 30);
    let all = p.every_id();

    for (key, values) in [
        ("status", STATUSES),
        ("type", TYPES),
        ("priority", PRIORITIES),
        ("milestone", &["v0.1", "v0.2"][..]),
    ] {
        for value in values {
            let yes = p.matching(&format!("{key}={value}"));
            let no = p.matching(&format!("{key}!={value}"));

            let both: Vec<_> = yes.intersection(&no).collect();
            assert!(
                both.is_empty(),
                "`{key}={value}` and `{key}!={value}` both match {both:?}"
            );

            let union: BTreeSet<u32> = yes.union(&no).copied().collect();
            let missing: Vec<_> = all.difference(&union).collect();
            assert!(
                missing.is_empty(),
                "{missing:?} match neither `{key}={value}` nor `{key}!={value}`"
            );
        }
    }
}

/// `a|b` is the union of `a` and `b`. Nothing more and nothing less.
#[test]
fn alternation_is_union() {
    let p = backlog(0xA17, 30);

    for (key, values) in [("status", STATUSES), ("type", TYPES)] {
        for a in values {
            for b in values {
                let left = p.matching(&format!("{key}={a}"));
                let right = p.matching(&format!("{key}={b}"));
                let together = p.matching(&format!("{key}={a}|{b}"));
                let expected: BTreeSet<u32> = left.union(&right).copied().collect();
                assert_eq!(
                    together, expected,
                    "`{key}={a}|{b}` is not the union of its alternatives"
                );
            }
        }
    }
}

/// A comma is intersection, and therefore commutative: the order clauses are
/// written in cannot change what they select.
#[test]
fn a_comma_is_intersection_and_does_not_care_about_order() {
    let p = backlog(0xC0A, 30);

    for status in STATUSES {
        for kind in TYPES {
            let a = format!("status={status}");
            let b = format!("type={kind}");
            let left = p.matching(&a);
            let right = p.matching(&b);
            let expected: BTreeSet<u32> = left.intersection(&right).copied().collect();

            let forwards = p.matching(&format!("{a},{b}"));
            let backwards = p.matching(&format!("{b},{a}"));

            assert_eq!(forwards, expected, "`{a},{b}` is not an intersection");
            assert_eq!(
                forwards, backwards,
                "`{a},{b}` and `{b},{a}` select different items"
            );
        }
    }
}

/// An empty value means unset, so it and its negation must also partition —
/// and `key=` must be exactly the items `key!=` every real value.
#[test]
fn an_empty_value_selects_exactly_the_items_without_the_field() {
    let p = backlog(0xE0F, 30);
    let all = p.every_id();

    for (key, values) in [("priority", PRIORITIES), ("milestone", &["v0.1"][..])] {
        let unset = p.matching(&format!("{key}="));

        // Everything with any real value, gathered by hand.
        let mut set: BTreeSet<u32> = BTreeSet::new();
        for value in values {
            set.extend(p.matching(&format!("{key}={value}")));
        }

        assert!(
            unset.intersection(&set).next().is_none(),
            "an item is both unset and set for `{key}`"
        );
        let union: BTreeSet<u32> = unset.union(&set).copied().collect();
        assert_eq!(
            union, all,
            "`{key}=` plus every value does not account for the whole backlog"
        );
    }
}
