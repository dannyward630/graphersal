//! The randomized differential test of verified backups (BK1 matrix row R8): seeded random
//! histories (every mutation kind, checkpoints, prunes, rollbacks, attic restores, marks,
//! random chunk/segment/WAL sizes, codecs and damage policies) with random backups (full,
//! increment, ZIP) and random damage. Every backup without damage equals the live graph (the
//! oracle); a backup of a damaged copy of the store is refused as the store's damage with
//! nothing kept, or equals the oracle (damage it does not read, a redundant copy); damage of
//! the live store's files is reported, applies the store's damage policy, and a backup from
//! memory then saves the graph without one write to the store. Never a panic.
//!
//! A fast budget runs in every `cargo test`. The long run is a release gate (CLAUDE.md):
//!
//! ```text
//! BACKUP_FUZZ_CASES=2000 BACKUP_FUZZ_SEED=1 BACKUP_FUZZ_THREADS=8 \
//!   cargo test --release -p graphersal --features script,io,display,serde,persist,persist-zip \
//!   --test all backup_fuzz_tests::
//! ```
//!
//! A failing case prints its seed; `BACKUP_FUZZ_REPLAY=<seed>` runs that case alone.
#![cfg(all(feature = "persist", feature = "script"))]

use std::io;
use std::sync::Arc;

use graphersal::persist::{
    BackupMode, Codec, DamagePolicy, DamageSide, MemDir, PersistError, RecoveryTarget,
    RollbackMode, Store, StoreDir, StoreOptions,
};

use crate::merge_checkpoint_tests::{Faults, Hooked, Rng, exact, fake_data, history, listing};

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn shared(dir: &MemDir) -> Arc<dyn StoreDir> {
    Arc::new(dir.clone())
}

/// The latest state of the store or backup in `dir`.
fn loaded(dir: &MemDir) -> Vec<String> {
    exact(
        &Store::open_read_only(shared(dir), RecoveryTarget::Latest)
            .unwrap()
            .graph,
    )
}

/// The files of the store a backup reads, in a copy: every file but the attic's.
fn damage_targets(dir: &MemDir) -> Vec<String> {
    dir.file_names()
        .into_iter()
        .filter(|n| !n.starts_with("attic/") && !n.ends_with("LOCK") && !n.ends_with("BACKUP"))
        .collect()
}

/// One case: everything follows from `seed`.
fn case(seed: u64) {
    let mut rng = Rng(seed);
    let policy = [DamagePolicy::Maintenance, DamagePolicy::Continue][rng.below(2)];
    let options = StoreOptions::new()
        .with_checkpoint_after_bytes(0)
        .with_codec([Codec::None, Codec::Lz4][rng.below(2)])
        .with_chunk_bytes(200 + rng.below(2_000) as u32)
        .with_snapshot_segment_bytes(800 + rng.below(20_000) as u64)
        .with_wal_segment_bytes(400 + rng.below(3_000) as u64)
        .with_damage_policy(policy);
    let inner = MemDir::new();
    let faults = Faults::new(true, io::ErrorKind::PermissionDenied);
    let dir: Arc<dyn StoreDir> = Arc::new(Hooked {
        inner: inner.clone(),
        hook: faults.clone(),
    });
    let store = Store::create(&dir, options.clone()).unwrap();
    fake_data(store.graph(), seed, 4 + rng.below(30));
    let mut kept = MemDir::new();
    let mut round = 0;
    for _ in 0..3 + rng.below(8) {
        let rounds = 1 + rng.below(20);
        history(store.graph(), &mut rng, rounds, round);
        round += rounds;
        let live = exact(&store.graph().read());
        let action = rng.below(9);
        if std::env::var("BACKUP_FUZZ_VERBOSE").is_ok() {
            eprintln!(
                "action {action} at commit {} lineage {:?} wal {:?} kept wal {:?}",
                store.graph().read().last_commit_seq(),
                store.info().unwrap().lineage.len(),
                inner
                    .file_names()
                    .into_iter()
                    .filter(|n| n.starts_with("wal/"))
                    .collect::<Vec<_>>(),
                kept.file_names()
                    .into_iter()
                    .filter(|n| n.starts_with("wal/") || n.starts_with("snapshots/"))
                    .collect::<Vec<_>>()
            );
        }
        match action {
            0 => {
                let target = MemDir::new();
                store.backup(&target).unwrap();
                assert_eq!(loaded(&target), live, "full");
                assert!(Store::verify_dir(&target).unwrap().is_ok());
            }
            1 | 2 => {
                increment(&store, &mut kept);
                assert_eq!(loaded(&kept), live, "increment");
            }
            3 => {
                // A ZIP backup with `persist-zip`, else one more increment.
                #[cfg(feature = "persist-zip")]
                {
                    let mut archive = io::Cursor::new(Vec::new());
                    store.backup_zip(&mut archive).unwrap();
                    let restored = MemDir::new();
                    Store::restore_zip(archive.get_ref().as_slice(), &restored).unwrap();
                    assert_eq!(loaded(&restored), live, "zip");
                }
                #[cfg(not(feature = "persist-zip"))]
                {
                    increment(&store, &mut kept);
                    assert_eq!(loaded(&kept), live, "increment");
                }
            }
            4 => {
                store.checkpoint(None).unwrap();
                if rng.below(2) == 0 {
                    let last = store.snapshots().unwrap().pop().unwrap().commit_seq;
                    store.prune(last).unwrap();
                }
            }
            5 => {
                let current = store.graph().read().last_commit_seq();
                let target = rng.below(current as usize + 1) as u64;
                if let Ok(report) =
                    store.rollback_to(RecoveryTarget::CommitSeq(target), RollbackMode::Move)
                    && rng.below(2) == 0
                    && let Some(attic) = report.attic
                {
                    store.attic_restore(&attic).unwrap();
                }
            }
            6 | 7 => {
                // Damage one byte of a COPY of the store's files, then back the copy up (by
                // another process: no durable end), full or as an increment of a copy of the
                // kept backup.
                let image = inner.deep_copy();
                let files = damage_targets(&image);
                let file = &files[rng.below(files.len())];
                let mut bytes = image.file(file).unwrap();
                if bytes.is_empty() {
                    continue;
                }
                let at = rng.below(bytes.len());
                bytes[at] ^= 1 << rng.below(8);
                image.put_file(file, bytes);
                let label = format!("{file} byte {at}");
                let increment = !kept.file_names().is_empty() && rng.below(2) == 0;
                let target = if increment {
                    kept.deep_copy()
                } else {
                    MemDir::new()
                };
                let before = listing(&target);
                match Store::backup_dir(&image, &target) {
                    Err(PersistError::BackupDamaged(damage)) => {
                        assert_eq!(damage.side, DamageSide::Store, "{label}");
                        assert_eq!(listing(&target), before, "{label}: target changed");
                    }
                    // An increment the damaged copy cannot continue (a damaged header of the
                    // backup's last segment in the store, a lineage it does not match).
                    Err(PersistError::BackupRefused { .. }) => {
                        assert_eq!(listing(&target), before, "{label}: target changed");
                    }
                    Ok(_) => assert_eq!(loaded(&target), live, "{label}"),
                    Err(other) => panic!("{label}: {other}"),
                }
            }
            _ => {
                // Damage of the live store's newest WAL segment: found, the policy applies, a
                // backup from memory saves the graph without one write to the store.
                let names: Vec<String> = inner
                    .file_names()
                    .into_iter()
                    .filter(|n| n.starts_with("wal/") && n.ends_with(".wal"))
                    .collect();
                let Some(last) = names.iter().max() else {
                    continue;
                };
                let mut bytes = inner.file(last).unwrap();
                if bytes.len() <= 64 {
                    continue;
                }
                let at = rng.below(bytes.len());
                bytes[at] ^= 1 << rng.below(8);
                inner.put_file(last, bytes);
                let found = match store.backup(MemDir::new()) {
                    Err(PersistError::BackupDamaged(damage)) => {
                        assert_eq!(damage.side, DamageSide::Store, "{last} byte {at}");
                        true
                    }
                    Err(other) => panic!("{last} byte {at}: {other}"),
                    Ok(_) => false,
                };
                assert!(found, "{last} byte {at}: damage not found");
                assert_eq!(
                    store.is_frozen_by_damage(),
                    policy == DamagePolicy::Maintenance
                );
                faults.arm(u64::MAX);
                let saved = MemDir::new();
                store.backup_from_memory(&saved, BackupMode::Auto).unwrap();
                assert_eq!(loaded(&saved), live, "from memory");
                if !kept.file_names().is_empty() {
                    match store.backup_from_memory(&kept, BackupMode::Auto) {
                        Ok(_) => assert_eq!(loaded(&kept), live, "from memory into the kept"),
                        // The store was rolled back behind the kept backup.
                        Err(PersistError::BackupRefused { reason, .. }) => {
                            assert!(reason.contains("after the graph in memory"), "{reason}")
                        }
                        Err(other) => panic!("from memory into the kept backup: {other}"),
                    }
                }
                assert!(
                    faults.ops.lock().unwrap().is_empty(),
                    "writes to the damaged store: {:?}",
                    faults.ops.lock().unwrap()
                );
                faults.disarm();
                return;
            }
        }
    }
    // The end: the store closes, reopens to the same graph and verifies; a last increment
    // equals it.
    let live = exact(&store.graph().read());
    store.close().unwrap();
    let reopened = Store::open(&dir, options).unwrap();
    assert_eq!(exact(&reopened.graph().read()), live, "reopened");
    let verify = reopened.verify().unwrap();
    let problems: Vec<_> = verify
        .problems
        .iter()
        .filter(|p| !(p.path.starts_with("attic/") && p.reason.contains("it continues is missing")))
        .collect();
    assert!(problems.is_empty(), "{problems:?}");
    if !kept.file_names().is_empty() {
        increment(&reopened, &mut kept);
        assert_eq!(loaded(&kept), live, "last increment");
    }
}

/// An increment of `kept`; when the store was rolled back behind it or pruned the WAL it
/// needs (the documented refusals), a full backup into a new directory takes its place.
fn increment(store: &Store, kept: &mut MemDir) {
    match store.backup(&*kept) {
        Ok(_) => {}
        Err(PersistError::BackupRefused { reason, .. }) => {
            assert!(
                reason.contains("rolled back in place")
                    || reason.contains("full backup")
                    || reason.contains("(--full)"),
                "{reason}"
            );
            *kept = MemDir::new();
            store.backup(&*kept).unwrap();
        }
        Err(other) => panic!("increment: {other}"),
    }
}

fn run_case(seed: u64) {
    if std::panic::catch_unwind(|| case(seed)).is_err() {
        panic!("backup fuzz case {seed} failed; replay it with BACKUP_FUZZ_REPLAY={seed}");
    }
}

/// The campaign: `BACKUP_FUZZ_CASES` cases from `BACKUP_FUZZ_SEED` on, on
/// `BACKUP_FUZZ_THREADS` threads (a small budget by default).
#[test]
fn random_histories_with_random_backups_and_damage() {
    if let Ok(seed) = std::env::var("BACKUP_FUZZ_REPLAY") {
        run_case(seed.parse().expect("BACKUP_FUZZ_REPLAY=<seed>"));
        return;
    }
    let cases = env_u64("BACKUP_FUZZ_CASES", 60);
    let first = env_u64("BACKUP_FUZZ_SEED", 1);
    let threads = env_u64("BACKUP_FUZZ_THREADS", 4).max(1);
    std::thread::scope(|scope| {
        for lane in 0..threads {
            scope.spawn(move || {
                let mut seed = first + lane;
                while seed < first + cases {
                    run_case(seed);
                    seed += threads;
                }
            });
        }
    });
}
