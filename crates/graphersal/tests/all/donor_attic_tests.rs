//! Repair donors under segment references and whole attic entries (the PF1 task; format spec
//! 11.1, 11.2, 12.3, 12.4, 13.1), through the public API:
//!
//! - **Donors.** After every checkpoint generation, damage in ANY segment file the newest
//!   snapshot uses (its own files and the twins it references in older snapshot directories or
//!   holders) opens the store in maintenance mode with the previous snapshot as the donor, and
//!   `repair_to` rebuilds the exact state without loss: the newest snapshot never shares a file
//!   with the previous one.
//! - **Attic.** Chains of rollbacks to ever earlier commits (before older entries' bases and
//!   before the files their snapshots reference), prunes between them, move and delete mode,
//!   on every backend: every entry forks to the state it took, restores (when allowed) in any
//!   order, a refused restore changes nothing; a directory that fails at every write step of
//!   such a rollback leaves a store that opens, verifies and keeps every entry whole.
#![cfg(all(feature = "persist", feature = "script"))]

use std::io;
use std::sync::Arc;

use graphersal::persist::{
    Codec, DamageKind, Donor, MemDir, RecoveryTarget, RollbackMode, Store, StoreDir, StoreOptions,
};

use crate::merge_checkpoint_tests::{
    Faults, Hooked, Rng, exact, fake_data, history, inputs, listing, oracle, run, shared, small,
};

/// The store verifies: no damage, no segment without a donor.
fn assert_clean(dir: &Arc<dyn StoreDir>, label: &str) {
    let report = Store::verify_dir(dir).unwrap();
    assert!(report.problems.is_empty(), "{label}: {:?}", report.problems);
    assert!(
        report.without_donor.is_empty(),
        "{label}: {:?}",
        report.without_donor
    );
}

// ---------------------------------------------------------------------------------------------
// Donors
// ---------------------------------------------------------------------------------------------

/// Sunny and rainy day of the donor rule: six checkpoint generations of a few touched ids
/// (unchanged ranges alternate between two files: copies, then references to twins), each
/// followed by damage of every segment file the newest snapshot uses, one at a time, in a
/// copy: maintenance mode names the previous snapshot as the donor, and the repair is
/// lossless and equals the oracle. Prunes now and then: the twins then live in holders.
#[test]
fn damage_of_any_file_of_the_newest_snapshot_repairs_from_the_previous_one() {
    let (options, _) = small(Codec::Lz4);
    let dir = MemDir::new();
    let store = Store::create(&dir, options.clone()).unwrap();
    fake_data(store.graph(), 21, 60);
    store.checkpoint(None).unwrap();
    store.close().unwrap();
    let mut references = 0;
    let mut copies = 0;
    for generation in 0..6 {
        let store = Store::open(&dir, options.clone()).unwrap();
        run(
            store.graph(),
            &format!(
                r#"g.v("p{}").property("gen", {generation}).to_list()"#,
                generation * 9
            ),
        );
        store.close().unwrap();
        let report = Store::checkpoint_dir(&dir, options.clone(), None).unwrap();
        assert!(report.merged, "{report:?}");
        references += report.segments_reused;
        copies += report.segments_copied;
        if generation % 2 == 1 {
            let store = Store::open(&dir, options.clone()).unwrap();
            let last = store.snapshots().unwrap().pop().unwrap().commit_seq;
            store.prune(last).unwrap();
            store.close().unwrap();
        }
        let label = format!("generation {generation}");
        assert_clean(&shared(&dir), &label);
        let expected = exact(&oracle(&shared(&dir)));
        let snapshots = Store::snapshots_dir(&dir).unwrap();
        let previous = snapshots[snapshots.len() - 2].commit_seq;
        let newest = snapshots[snapshots.len() - 1].commit_seq;
        let files: Vec<String> = inputs(&dir)
            .into_iter()
            .filter(|f| f.ends_with(".seg"))
            .collect();
        assert!(files.len() > 3, "{label}: {files:?}");
        for file in &files {
            let image = dir.deep_copy();
            let mut bytes = image.file(file).unwrap();
            // Inside the first chunk's stored bytes (after the 64-byte header and the 17-byte
            // chunk header).
            bytes[64 + 17 + 3] ^= 0x10;
            image.put_file(file, bytes);
            let damaged = Store::open(&image, options.clone()).unwrap();
            let report = damaged
                .maintenance()
                .unwrap_or_else(|| panic!("{label} {file}: damage went unnoticed"));
            let chunk = report
                .damage
                .iter()
                .find(|d| d.kind == DamageKind::Chunk && d.snapshot == Some(newest))
                .unwrap_or_else(|| panic!("{label} {file}: {report}"));
            assert!(
                matches!(chunk.donor, Some(Donor::OlderSnapshot { snapshot, .. }) if snapshot == previous),
                "{label} {file}: {report}"
            );
            let repaired = MemDir::new();
            let repair = damaged.repair_to(&repaired).unwrap();
            assert!(repair.is_lossless(), "{label} {file}: {repair}");
            let opened = Store::open(&repaired, StoreOptions::new()).unwrap();
            assert_eq!(
                exact(&opened.graph().read())[1..],
                expected[1..],
                "{label} {file}"
            );
        }
    }
    assert!(references > 10, "{references}");
    assert!(copies > 10, "{copies}");
}

// ---------------------------------------------------------------------------------------------
// Attic
// ---------------------------------------------------------------------------------------------

/// The backends of the attic tests: memory, a directory, a single file.
fn backends(base: &std::path::Path, name: &str) -> Vec<Arc<dyn StoreDir>> {
    vec![
        Arc::new(MemDir::new()),
        Arc::new(graphersal::persist::FsDir::new(
            base.join(format!("dir-{name}")),
        )),
        Arc::new(graphersal::persist::SingleFileDir::new(
            base.join(format!("{name}.gstore")),
        )),
    ]
}

/// A store with three checkpoints after Fake data and histories between them (snapshots that
/// reference twins), closed: `(snapshot positions, the position)`.
fn build(dir: &Arc<dyn StoreDir>, options: &StoreOptions, seed: u64) -> (Vec<u64>, u64) {
    let store = Store::create(dir, options.clone()).unwrap();
    fake_data(store.graph(), seed, 40);
    let mut rng = Rng(seed);
    for round in 0..3 {
        store.checkpoint(None).unwrap();
        history(store.graph(), &mut rng, 6, round * 10);
    }
    store.checkpoint(None).unwrap();
    history(store.graph(), &mut rng, 4, 100);
    let position = store.graph().read().last_commit_seq();
    let snapshots = store
        .snapshots()
        .unwrap()
        .iter()
        .map(|s| s.commit_seq)
        .collect();
    store.close().unwrap();
    (snapshots, position)
}

/// The state an attic entry must hold: `(commit, exact state without the identity line)`.
type State = (u64, Vec<String>);

fn state_of(store: &Store) -> State {
    let graph = store.graph().read();
    (graph.last_commit_seq(), exact(&graph)[1..].to_vec())
}

/// Every entry forks to its state.
fn check_forks(store: &Store, states: &[(String, State)], label: &str) {
    for (id, (commit, state)) in states {
        let fork = MemDir::new();
        store
            .attic_fork(id, &fork)
            .unwrap_or_else(|err| panic!("{label}: fork of {id}: {err}"));
        let forked = Store::open(&fork, StoreOptions::new()).unwrap();
        let graph = forked.graph().read();
        assert_eq!(graph.last_commit_seq(), *commit, "{label}: fork of {id}");
        assert_eq!(exact(&graph)[1..], state[..], "{label}: fork of {id}");
    }
}

/// Rolls back to ever earlier commits — before the newest snapshot, before an older entry's
/// base, before the files its snapshots reference — with a prune after each; with `commits`,
/// new commits and a checkpoint follow each rollback (older entries are then fork-only).
/// Returns the entries with their states.
fn rollback_chain(
    store: &Store,
    snapshots: &[u64],
    commits: bool,
    mode_of: impl Fn(usize) -> RollbackMode,
    label: &str,
) -> Vec<(String, State)> {
    let mut states = Vec::new();
    let targets = [
        snapshots[snapshots.len() - 1] - 1,
        snapshots[snapshots.len() - 2] - 1,
        snapshots[snapshots.len() - 3].saturating_sub(2),
    ];
    let mut rng = Rng(7);
    for (step, target) in targets.into_iter().enumerate() {
        let before = state_of(store);
        let report = store
            .rollback_to(RecoveryTarget::CommitSeq(target), mode_of(step))
            .unwrap_or_else(|err| panic!("{label}: rollback to {target}: {err}"));
        if let Some(id) = report.attic {
            states.push((id, before));
        }
        assert_clean(
            store.dir(),
            &format!("{label}: after the rollback to {target}"),
        );
        check_forks(store, &states, label);
        // A prune that keeps what the next, earlier rollback needs; the entries keep their
        // bases.
        store.prune(snapshots[1]).unwrap();
        assert_clean(store.dir(), &format!("{label}: after a prune at {target}"));
        check_forks(store, &states, label);
        if commits {
            history(store.graph(), &mut rng, 5, 500 + step * 10);
            store.checkpoint(None).unwrap();
        }
    }
    states
}

/// Restores the entries in `order`, retrying the refused ones until none restores: a refused
/// restore changes no file (checked on a store in memory); returns how many were restored.
fn restore_in_order(
    store: &Store,
    order: &[String],
    states: &[(String, State)],
    memory: Option<&MemDir>,
    label: &str,
) -> usize {
    let mut left: Vec<String> = order.to_vec();
    let mut restored = 0;
    loop {
        let mut progress = false;
        for id in left.clone() {
            if store.attic_can_restore(&id).is_ok() {
                let position = store.attic_restore(&id).unwrap();
                let (_, (commit, state)) = states.iter().find(|(e, _)| *e == id).unwrap();
                assert_eq!(position, *commit, "{label}: restore of {id}");
                assert_eq!(
                    exact(&store.graph().read())[1..],
                    state[..],
                    "{label}: restore of {id}"
                );
                assert_clean(store.dir(), &format!("{label}: after restoring {id}"));
                left.retain(|e| *e != id);
                restored += 1;
                progress = true;
            } else {
                let before = memory.map(listing);
                assert!(store.attic_restore(&id).is_err(), "{label}: {id}");
                if let Some(dir) = memory {
                    assert_eq!(
                        Some(listing(dir)),
                        before,
                        "{label}: a refused restore of {id}"
                    );
                }
            }
        }
        if !progress {
            return restored;
        }
    }
}

/// Every permutation of `items`.
fn permutations(items: &[String]) -> Vec<Vec<String>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut out = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first.clone());
            out.push(tail);
        }
    }
    out
}

/// Chains of rollbacks to ever earlier commits with prunes between, on every backend: every
/// entry forks to the state it took; without commits between the rollbacks every entry
/// restores, in every order (a store in memory tries all six; the refused ones change nothing),
/// back to the original history.
#[test]
fn chains_of_rollbacks_to_ever_earlier_commits_keep_every_entry_whole() {
    let base = std::env::temp_dir().join(format!("graphersal-pf1-attic-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let (options, _) = small(Codec::Lz4);
    let options = options.with_wal_segment_bytes(900);
    for (index, dir) in backends(&base, "chain").into_iter().enumerate() {
        let label = dir.location();
        let (snapshots, position) = build(&dir, &options, 31);
        assert!(snapshots.len() >= 4, "{label}: {snapshots:?}");
        let store = Store::open(&dir, options.clone()).unwrap();
        let original = state_of(&store);
        assert_eq!(original.0, position);
        let states = rollback_chain(&store, &snapshots, false, |_| RollbackMode::Move, &label);
        assert_eq!(states.len(), 3, "{label}");
        drop(store);
        let ids: Vec<String> = states.iter().map(|(id, _)| id.clone()).collect();
        // Every order on copies of the store in memory; the given order on this backend.
        let orders = if index == 0 {
            permutations(&ids)
        } else {
            vec![ids.clone()]
        };
        for order in orders {
            let label = format!("{label}: order {order:?}");
            let (store, memory) = if index == 0 {
                let image = MemDir::new();
                copy_into(&dir, &image);
                (Store::open(&image, options.clone()).unwrap(), Some(image))
            } else {
                (Store::open(&dir, options.clone()).unwrap(), None)
            };
            let restored = restore_in_order(&store, &order, &states, memory.as_ref(), &label);
            assert_eq!(restored, 3, "{label}");
            assert_eq!(state_of(&store), original, "{label}");
            assert!(store.attic_list().unwrap().is_empty(), "{label}");
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// Copies every file of `from` (a store in memory) into `to`.
fn copy_into(from: &Arc<dyn StoreDir>, to: &MemDir) {
    for name in walk(from.as_ref(), "") {
        to.put_file(&name, from.read(&name, u64::MAX).unwrap());
    }
}

/// Every file name below `dir` in `store`.
fn walk(store: &dyn StoreDir, dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    for entry in store.list(dir).unwrap_or_default() {
        let name = if dir.is_empty() {
            entry.name.clone()
        } else {
            format!("{dir}/{}", entry.name)
        };
        match entry.kind {
            graphersal::persist::EntryKind::Dir => out.extend(walk(store, &name)),
            _ => out.push(name),
        }
    }
    out
}

/// With commits and checkpoints after each rollback, and delete mode in between: the older
/// entries cannot be restored any more (refused, nothing changed) but every one forks to its
/// state; removing entries in any order keeps the others whole, on every backend.
#[test]
fn rollback_chains_with_commits_between_and_delete_mode_keep_forks_whole() {
    let base = std::env::temp_dir().join(format!("graphersal-pf1-forks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let (options, _) = small(Codec::None);
    for dir in backends(&base, "forks") {
        let label = dir.location();
        let (snapshots, _) = build(&dir, &options, 32);
        let store = Store::open(&dir, options.clone()).unwrap();
        let modes = |step: usize| {
            if step == 1 {
                RollbackMode::Delete
            } else {
                RollbackMode::Move
            }
        };
        let mut states = rollback_chain(&store, &snapshots, true, modes, &label);
        assert_eq!(states.len(), 2, "{label}");
        for (id, _) in &states {
            assert!(store.attic_can_restore(id).is_err(), "{label}: {id}");
        }
        // Remove the older entry: the newer one stays whole.
        let (removed, _) = states.remove(0);
        store.attic_remove(&removed).unwrap();
        assert_clean(store.dir(), &label);
        check_forks(&store, &states, &label);
        drop(store);
        let reopened = Store::open(&dir, options.clone()).unwrap();
        check_forks(&reopened, &states, &label);
        assert_clean(reopened.dir(), &label);
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// A directory that fails at every write step of a rollback before an older entry's base
/// (a crash: every later step fails too; a transient I/O error; a full disk): the store image
/// opens (completing the rollback when its INTENT was written), verifies, and every attic
/// entry forks to its state; the position is the old one or the target.
#[test]
fn a_failing_directory_at_every_step_of_a_rebasing_rollback_keeps_every_entry_whole() {
    for (crash, kind) in [
        (true, io::ErrorKind::Other),
        (false, io::ErrorKind::Other),
        (false, io::ErrorKind::StorageFull),
    ] {
        for at in 0.. {
            let (options, _) = small(Codec::Lz4);
            let options = options.with_wal_segment_bytes(700);
            let inner = MemDir::new();
            let faults = Faults::new(crash, kind);
            let dir: Arc<dyn StoreDir> = Arc::new(Hooked {
                inner: inner.clone(),
                hook: faults.clone(),
            });
            let (snapshots, _) = build(&dir, &options, 33);
            let store = Store::open(&dir, options.clone()).unwrap();
            let newest = snapshots[snapshots.len() - 1];
            let first_state = state_of(&store);
            let first = store
                .rollback_to(RecoveryTarget::CommitSeq(newest - 1), RollbackMode::Move)
                .unwrap()
                .attic
                .unwrap();
            let second_state = state_of(&store);
            let target = snapshots[snapshots.len() - 3].saturating_sub(2);
            let label = format!("crash {crash} {kind:?} step {at}");
            faults.arm(at);
            let result = store.rollback_to(RecoveryTarget::CommitSeq(target), RollbackMode::Move);
            faults.disarm();
            let fired = faults.fired();
            if !fired {
                result.unwrap_or_else(|err| panic!("{label}: {err}"));
                assert!(at > 10, "{label}: a rollback takes many writes");
                break;
            }
            // A crash: the disk as the failure left it (the process is gone). Otherwise the
            // process carries on and closes the store.
            let image = if crash {
                inner.deep_copy()
            } else {
                inner.clone()
            };
            drop(store);
            let image = shared(&image);
            let reopened = Store::open(&image, options.clone())
                .unwrap_or_else(|err| panic!("{label}: reopen: {err}"));
            assert_clean(&image, &label);
            let entries = reopened.attic_list().unwrap();
            let mut states = vec![(first.clone(), first_state.clone())];
            let position = reopened.graph().read().last_commit_seq();
            if entries.len() == 2 {
                assert_eq!(position, target, "{label}");
                let second = entries.iter().find(|e| e.id != first).unwrap().id.clone();
                states.push((second, second_state.clone()));
            } else {
                assert_eq!(entries.len(), 1, "{label}");
                assert_eq!(position, second_state.0, "{label}");
            }
            check_forks(&reopened, &states, &label);
        }
    }
}

/// Found by the long randomized run: a checkpoint of the CLOSED store right after a rollback
/// writes a snapshot of the new lineage at the target and no WAL record, so the rollback can
/// still be undone; the restore removes that snapshot (the old lineage's history replaces it)
/// and the store is exactly where it was, on every backend.
#[test]
fn a_restore_after_a_closed_checkpoint_at_the_target_drops_the_new_lineages_snapshot() {
    let base = std::env::temp_dir().join(format!("graphersal-pf1-closed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let (options, _) = small(Codec::Lz4);
    for dir in backends(&base, "closed") {
        let label = dir.location();
        let (snapshots, _) = build(&dir, &options, 34);
        let store = Store::open(&dir, options.clone()).unwrap();
        let before = state_of(&store);
        let target = snapshots[snapshots.len() - 1] - 1;
        let id = store
            .rollback_to(RecoveryTarget::CommitSeq(target), RollbackMode::Move)
            .unwrap()
            .attic
            .unwrap();
        store.close().unwrap();
        let report = Store::checkpoint_dir(&dir, options.clone(), None).unwrap();
        assert!(
            report.written && report.snapshot.commit_seq == target,
            "{label}"
        );
        let store = Store::open(&dir, options.clone()).unwrap();
        assert_eq!(store.attic_restore(&id).unwrap(), before.0, "{label}");
        assert_eq!(state_of(&store), before, "{label}");
        assert_clean(store.dir(), &label);
        drop(store);
        let reopened = Store::open(&dir, options.clone()).unwrap();
        assert_eq!(state_of(&reopened), before, "{label}");
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// The restore order (user decision 2026-10-09): after several rollbacks, attic entries
/// restore newest first. Restoring an older entry first is refused, changes nothing and names
/// the newer entry to restore before it (or a fork); after the newer one, the older one
/// restores. Any entry forks at any time.
#[test]
fn attic_entries_restore_newest_first_and_the_refusal_names_the_newer_entry() {
    let dir = MemDir::new();
    let store = Store::create(&dir, small(Codec::None).0).unwrap();
    let mut rng = Rng(11);
    history(store.graph(), &mut rng, 12, 0);
    let top = store.graph().read().last_commit_seq();
    let older = store
        .rollback_to(RecoveryTarget::CommitSeq(top - 2), RollbackMode::Move)
        .unwrap()
        .attic
        .unwrap();
    let newer = store
        .rollback_to(RecoveryTarget::CommitSeq(top - 5), RollbackMode::Move)
        .unwrap()
        .attic
        .unwrap();
    let before = listing(&dir);
    let refused = store.attic_restore(&older).unwrap_err().to_string();
    assert!(refused.contains("newest first"), "{refused}");
    assert!(
        refused.contains(&format!("restore the newer entry {newer} first")),
        "{refused}"
    );
    assert!(refused.contains("fork the entry instead"), "{refused}");
    assert_eq!(listing(&dir), before, "a refused restore changes nothing");
    // Any entry forks at any time.
    let fork = MemDir::new();
    store.attic_fork(&older, &fork).unwrap();
    assert_eq!(store.attic_restore(&newer).unwrap(), top - 2);
    assert_eq!(store.attic_restore(&older).unwrap(), top);
}
