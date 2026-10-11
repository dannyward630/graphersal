//! Hostile-input robustness of Store directories and WAL streams (CLAUDE.md rule 1, format spec
//! sections 9 and 13): a Store on a [`MemDir`] with every kind of file (GRAPH and its copy,
//! several WAL segments, two snapshots, marks, an attic) is damaged at random (cuts, flipped
//! bits, overwritten bytes, duplicated or removed slices, removed files, extra files, one
//! file's bytes under another name), then opened, opened read-only, verified, repaired, backed
//! up and inspected. A chain of journals is mutated the same way and recovered to every target
//! kind. Every case runs in [`guarded`] (own thread, panic catch, watchdog).
//!
//! The invariant beyond "no panic, no hang": a store that opens writable without a damage
//! report, a read-only load and a recovered journal hold a state the graph really had (one of
//! the recorded history states), never silently wrong data; anything else must be an error or
//! a damage report (maintenance mode). When a case only removes, empties, copies or adds whole
//! files (no byte of a file is cut or changed, so nothing looks like an interrupted write), a
//! writable open without a damage report holds the FINAL state: a lost, emptied or replaced
//! WAL segment is never a silently shorter history (format spec 9.8). A store that opens
//! writable must take a commit and reopen with it.
//!
//! `ROBUSTNESS_INPUT_CASES=<n>` raises the mutation budget like `robustness_input_tests`.
#![cfg(all(feature = "persist", feature = "script"))]

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use graphersal::persist::{
    self, Durability, Journal, MemDir, RecoveryTarget, RollbackMode, SharedBuffer, Store, StoreDir,
    StoreOptions, WriteOptions,
};
use graphersal::prelude::*;

use crate::persist_tests::dump;
use crate::robustness_fuzz_tests::{Rng, guarded};

const WATCHDOG: Duration = Duration::from_secs(60);

fn budget(default: u64) -> u64 {
    std::env::var("ROBUSTNESS_INPUT_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Byte mutations of one file's contents (like `persist_robustness_tests`).
fn mutate_bytes(rng: &mut Rng, base: &[u8]) -> Vec<u8> {
    let mut out = base.to_vec();
    for _ in 0..1 + rng.below(3) {
        if out.is_empty() {
            out.extend_from_slice(&[0xFF; 8][..rng.below(9)]);
            break;
        }
        let at = rng.below(out.len());
        match rng.below(7) {
            0 => out.truncate(at),
            1 | 2 => out[at] ^= 1 << rng.below(8),
            3 => out[at] = *rng.pick(&[0x00, 0xFF, 0x7F, 0x80, 0x01]),
            4 => {
                let end = (at + rng.below(64)).min(out.len());
                let slice = out[at..end].to_vec();
                out.splice(at..at, slice);
            }
            5 => {
                let end = (at + rng.below(32)).min(out.len());
                out.drain(at..end);
            }
            _ => {
                let end = (at + 8).min(out.len());
                for byte in &mut out[at..end] {
                    *byte = 0xFF;
                }
            }
        }
    }
    out
}

fn add_person(graph: &Graph, name: &str) {
    graph
        .write()
        .traversal_mut()
        .add_v("person")
        .property("name", name)
        .to_list()
        .unwrap();
}

/// A store with every kind of file, and every state its graph had (dumps).
struct Fixture {
    dir: MemDir,
    states: Arc<HashSet<Vec<String>>>,
    final_state: Vec<String>,
}

fn fixture() -> Fixture {
    let dir = MemDir::named("memory:fuzz");
    let options = StoreOptions::new()
        .with_wal_segment_bytes(600)
        .with_chunk_bytes(256)
        .with_snapshot_segment_bytes(1024);
    let store = Store::create_with(&dir, TraversalGraph::tinkerpop_modern(), options).unwrap();
    let mut states = HashSet::new();
    let record = |states: &mut HashSet<Vec<String>>, store: &Store| {
        states.insert(dump(&store.graph().read()));
    };
    record(&mut states, &store);
    for name in ["ann", "bob"] {
        add_person(store.graph(), name);
        record(&mut states, &store);
    }
    store.mark("m1").unwrap();
    store.checkpoint(Some("first")).unwrap();
    store
        .graph()
        .write()
        .traversal_mut()
        .v("1")
        .property("nested", ElementProperty::Uuid(7))
        .to_list()
        .unwrap();
    record(&mut states, &store);
    for name in ["cy", "dan", "eve"] {
        add_person(store.graph(), name);
        record(&mut states, &store);
    }
    store
        .rollback_to(RecoveryTarget::Mark("m1".into()), RollbackMode::Move)
        .unwrap();
    record(&mut states, &store);
    store
        .graph()
        .write()
        .traversal_mut()
        .v("2")
        .drop()
        .to_list()
        .unwrap();
    record(&mut states, &store);
    store.checkpoint(None).unwrap();
    for name in ["fay", "gus"] {
        add_person(store.graph(), name);
        record(&mut states, &store);
    }
    store.mark("m2").unwrap();
    add_person(store.graph(), "hal");
    record(&mut states, &store);
    let final_state = dump(&store.graph().read());
    // Not closed: the crash image of a running store (no clean-close flag).
    drop(store);
    Fixture {
        dir,
        states: Arc::new(states),
        final_state,
    }
}

/// Damages a copy of `dir` with 1..=3 random file-level mutations; returns the copy and what
/// was done.
fn damage(rng: &mut Rng, dir: &MemDir) -> (MemDir, Vec<String>) {
    let copy = dir.deep_copy();
    let mut done = Vec::new();
    for _ in 0..1 + rng.below(3) {
        let names = copy.file_names();
        if names.is_empty() {
            break;
        }
        let name = rng.pick(&names).clone();
        match rng.below(10) {
            0..=5 => {
                let bytes = copy.file(&name).unwrap_or_default();
                copy.put_file(&name, mutate_bytes(rng, &bytes));
                done.push(format!("mutate {name}"));
            }
            6 => {
                let _ = StoreDir::remove_file(&copy, &name);
                done.push(format!("remove {name}"));
            }
            7 => {
                // Another file's bytes under this name (a misdirected write).
                let other = rng.pick(&names).clone();
                let bytes = copy.file(&other).unwrap_or_default();
                copy.put_file(&name, bytes);
                done.push(format!("copy {other} -> {name}"));
            }
            8 => {
                // An extra file: a sibling name with garbage or a copy of a real file.
                let extra = match rng.below(6) {
                    0 => format!("{name}.extra"),
                    1 => "INTENT".to_string(),
                    2 => "tmp-x".to_string(),
                    3 => name.replace('0', "9"),
                    4 => name.replace('1', "2"),
                    _ => "snapshots/zzz/manifest".to_string(),
                };
                let bytes = if rng.chance(50) {
                    copy.file(&name).unwrap_or_default()
                } else {
                    let base = copy.file(&name).unwrap_or_default();
                    mutate_bytes(rng, &base)
                };
                copy.put_file(&extra, bytes);
                done.push(format!("extra {extra} from {name}"));
            }
            _ => {
                copy.put_file(&name, Vec::new());
                done.push(format!("empty {name}"));
            }
        }
    }
    (copy, done)
}

/// Every operation on a damaged directory; `Err` is a violated invariant (silently wrong data).
fn exercise_store(
    dir: MemDir,
    states: &HashSet<Vec<String>>,
    final_state: &[String],
    whole_files_only: bool,
) -> Result<&'static str, String> {
    let _ = Store::verify_dir(&dir);
    let _ = Store::damage_report_dir(&dir);
    let _ = Store::inspect(&dir);
    let _ = Store::snapshots_dir(&dir);
    let _ = Store::marks_dir(&dir);
    let _ = Store::attic_list_dir(&dir);
    let _ = Store::backup_marker_dir(&dir);
    let _ = Store::is_store(&dir);
    for target in [
        RecoveryTarget::Latest,
        RecoveryTarget::Mark("m1".into()),
        RecoveryTarget::Mark("m2".into()),
        RecoveryTarget::CommitSeq(3),
        RecoveryTarget::Time(i64::MAX),
    ] {
        if let Ok(view) = Store::open_read_only(&dir, target.clone()) {
            let state = dump(&view.graph);
            if !states.contains(&state) {
                return Err(format!(
                    "open_read_only({target}) gave a state the graph never had: {state:?}"
                ));
            }
        }
    }
    let repaired = MemDir::new();
    if let Ok(report) = Store::repair_dir(&dir, &repaired) {
        let _ = report.to_string();
        let store = Store::open(&repaired, StoreOptions::new())
            .map_err(|err| format!("the repaired store does not open: {err}"))?;
        if store.maintenance().is_some() {
            return Err("the repaired store opens in maintenance mode".into());
        }
    }
    let store = match Store::open(&dir, StoreOptions::new()) {
        Ok(store) => store,
        Err(err) => {
            let _ = err.to_string();
            return Ok("refused");
        }
    };
    let _ = store.info();
    let _ = store.verify();
    let _ = store.backup(MemDir::new());
    let _ = store.export_packed(None, &mut Vec::new());
    if let Some(report) = store.maintenance() {
        let _ = report.to_string();
        let _ = store.repair_to(MemDir::new());
        let _ = store.read_only_at(RecoveryTarget::Latest);
        return Ok("maintenance");
    }
    let state = dump(&store.graph().read());
    if !states.contains(&state) {
        return Err(format!(
            "a store without a damage report opened with a state the graph never had: {state:?}"
        ));
    }
    if whole_files_only && state != final_state {
        return Err(format!(
            "whole files were removed, emptied or replaced, and the store opened writable \
             without a damage report at an earlier state: {state:?}"
        ));
    }
    // A writable store takes a commit, closes, and reopens with it.
    let graph = store.graph().clone();
    let written = graph
        .write()
        .traversal_mut()
        .add_v("person")
        .property("name", "zed")
        .to_list();
    if written.is_err() {
        return Ok("read-only");
    }
    let expected = dump(&graph.read());
    drop(graph);
    if let Err(err) = store.close() {
        let _ = err.to_string();
        return Ok("close failed");
    }
    let reopened = Store::open(&dir, StoreOptions::new())
        .map_err(|err| format!("a store that took a commit does not reopen: {err}"))?;
    if reopened.maintenance().is_none() && dump(&reopened.graph().read()) != expected {
        return Err("a store reopened without the commit it took".into());
    }
    Ok("writable")
}

#[test]
fn damaged_store_directories_in_memory_never_crash_or_lie() {
    let fixture = fixture();
    // The undamaged image opens to the final state.
    {
        let copy = fixture.dir.deep_copy();
        let store = Store::open(&copy, StoreOptions::new()).unwrap();
        assert!(store.maintenance().is_none());
        assert_eq!(dump(&store.graph().read()), fixture.final_state);
    }
    let mut rng = Rng::new(0x5702_EF22);
    let mut fatal = Vec::new();
    let mut seen = std::collections::BTreeMap::<&str, u64>::new();
    for case in 0..budget(120) {
        let (copy, done) = damage(&mut rng, &fixture.dir);
        let states = fixture.states.clone();
        let final_state = fixture.final_state.clone();
        let whole_files_only = done.iter().all(|d| !d.starts_with("mutate "));
        let result = guarded(WATCHDOG, move || {
            exercise_store(copy, &states, &final_state, whole_files_only)
        });
        match result {
            Ok(Ok(kind)) => *seen.entry(kind).or_default() += 1,
            Ok(Err(lie)) => fatal.push(format!("case {case} {done:?}: {lie}")),
            Err(outcome) => fatal.push(format!("case {case} {done:?}: {outcome:?}")),
        }
    }
    assert!(fatal.is_empty(), "{}", fatal.join("\n"));
    eprintln!("damaged store outcomes: {seen:?}");
    // The fuzzer reaches every path: refused, maintenance mode and a healthy writable open.
    for kind in ["refused", "maintenance", "writable"] {
        assert!(seen.contains_key(kind), "no case was {kind}: {seen:?}");
    }
}

/// The finding of the first fuzz run (format spec 9.8): removing, emptying or replacing the
/// newest WAL segment of the fixture opened the store writable at an earlier commit without a
/// damage report. Now each is maintenance mode with the lost commits named.
#[test]
fn the_newest_wal_segment_removed_emptied_or_replaced_is_damage() {
    let fixture = fixture();
    let mut names: Vec<String> = fixture
        .dir
        .file_names()
        .into_iter()
        .filter(|n| n.starts_with("wal/"))
        .collect();
    names.sort();
    let newest = names.last().unwrap().clone();
    let older = names[names.len() - 2].clone();
    for what in ["remove", "empty", "replace"] {
        let copy = fixture.dir.deep_copy();
        match what {
            "remove" => StoreDir::remove_file(&copy, &newest).unwrap(),
            "empty" => copy.put_file(&newest, Vec::new()),
            _ => copy.put_file(&newest, copy.file(&older).unwrap()),
        }
        let store = Store::open(&copy, StoreOptions::new()).unwrap();
        let report = store
            .maintenance()
            .unwrap_or_else(|| panic!("{what} {newest}: opened writable"));
        assert!(
            report.damage.iter().any(|d| d.path == newest),
            "{what}: {report}"
        );
        assert!(!Store::verify_dir(&copy).unwrap().is_ok(), "{what}");
    }
}

/// A chain of journals (one per segment, the Store's WAL layout as streams): a base snapshot,
/// three journals, and every state after each commit.
fn journal_chain() -> (Vec<u8>, Vec<Vec<u8>>, HashSet<Vec<String>>) {
    let mut graph = TraversalGraph::tinkerpop_modern();
    let mut snapshot = Vec::new();
    persist::write_snapshot(&graph, &mut snapshot, &WriteOptions::new()).unwrap();
    let mut states = HashSet::new();
    states.insert(dump(&graph));
    let mut journals = Vec::new();
    for part in 0..3 {
        let wal = SharedBuffer::new();
        let mut detached = graph;
        Journal::new(wal.clone(), Durability::Os)
            .attach(&mut detached)
            .unwrap();
        for i in 0..3 {
            detached
                .traversal_mut()
                .add_v("person")
                .property("name", format!("p{part}-{i}"))
                .to_list()
                .unwrap();
            states.insert(dump(&detached));
        }
        detached.mark(&format!("mark{part}")).unwrap();
        detached
            .traversal_mut()
            .v("1")
            .property("age", part as i64)
            .to_list()
            .unwrap();
        states.insert(dump(&detached));
        journals.push(wal.contents());
        // A fresh journal continues on the graph (a new segment).
        let mut bytes = Vec::new();
        persist::write_snapshot(&detached, &mut bytes, &WriteOptions::new()).unwrap();
        graph = TraversalGraph::read_snapshot(bytes.as_slice()).unwrap();
    }
    (snapshot, journals, states)
}

#[test]
fn mutated_journal_chains_never_crash_or_lie() {
    let (snapshot, journals, states) = journal_chain();
    let states = Arc::new(states);
    let mut rng = Rng::new(0x3A1C_4A17);
    let mut fatal = Vec::new();
    let mut recovered = 0;
    for case in 0..budget(200) {
        let mut chain = journals.clone();
        let which = rng.below(chain.len() + 1);
        if which == chain.len() {
            // Reorder, drop or duplicate a journal.
            match rng.below(3) {
                0 => chain.swap(0, 2),
                1 => {
                    chain.remove(1);
                }
                _ => chain.insert(1, chain[1].clone()),
            }
        } else {
            chain[which] = mutate_bytes(&mut rng, &chain[which]);
        }
        let base = if rng.chance(10) {
            mutate_bytes(&mut rng, &snapshot)
        } else {
            snapshot.clone()
        };
        let target = match rng.below(5) {
            0 => RecoveryTarget::Latest,
            1 => RecoveryTarget::CommitSeq(rng.below(14) as u64),
            2 => RecoveryTarget::Mark(format!("mark{}", rng.below(4))),
            3 => RecoveryTarget::Time(rng.next_u64() as i64),
            _ => RecoveryTarget::Time(i64::MAX),
        };
        let states = states.clone();
        let result = guarded(WATCHDOG, move || -> Result<bool, String> {
            for journal in &chain {
                if let Ok(reader) = persist::read_journal(journal.as_slice()) {
                    let _ = reader.graph_id();
                    for record in reader {
                        if record.is_err() {
                            break;
                        }
                    }
                }
            }
            let mut recovery = persist::Recovery::new()
                .snapshot(base.as_slice())
                .target(target);
            for journal in &chain {
                recovery = recovery.journal(journal.as_slice());
            }
            if let Ok(recovered) = recovery.run() {
                let _ = format!("{:?}", recovered.report);
                let state = dump(&recovered.graph);
                if !states.contains(&state) {
                    return Err(format!("recovered a state the graph never had: {state:?}"));
                }
                return Ok(true);
            }
            Ok(false)
        });
        match result {
            Ok(Ok(true)) => recovered += 1,
            Ok(Ok(false)) => {}
            Ok(Err(lie)) => fatal.push(format!("case {case}: {lie}")),
            Err(outcome) => fatal.push(format!("case {case}: {outcome:?}")),
        }
    }
    assert!(fatal.is_empty(), "{}", fatal.join("\n"));
    assert!(recovered > 0, "no mutated chain recovered at all");
}

#[test]
fn mutated_packed_snapshots_never_create_a_store_that_lies() {
    let graph = TraversalGraph::tinkerpop_modern();
    let mut packed = Vec::new();
    persist::write_snapshot(
        &graph,
        &mut packed,
        &WriteOptions::new()
            .with_chunk_bytes(128)
            .with_segment_bytes(512),
    )
    .unwrap();
    let expected = dump(&graph);
    let mut rng = Rng::new(0x6A5A_9001);
    let mut fatal = Vec::new();
    for case in 0..budget(150) {
        let input = mutate_bytes(&mut rng, &packed);
        let expected = expected.clone();
        let result = guarded(WATCHDOG, move || -> Result<(), String> {
            let _ = persist::read_snapshot_info(input.as_slice());
            if let Ok(read) = TraversalGraph::read_snapshot(input.as_slice())
                && dump(&read) != expected
            {
                return Err("a damaged packed snapshot read as different data".into());
            }
            let dir = MemDir::new();
            if let Ok(store) =
                Store::create_from_packed(&dir, input.as_slice(), StoreOptions::new())
            {
                if dump(&store.graph().read()) != expected {
                    return Err("a store created from a damaged snapshot holds other data".into());
                }
                let _ = store.verify();
            }
            Ok(())
        });
        match result {
            Ok(Ok(())) => {}
            Ok(Err(lie)) => fatal.push(format!("case {case}: {lie}")),
            Err(outcome) => fatal.push(format!("case {case}: {outcome:?}")),
        }
    }
    assert!(fatal.is_empty(), "{}", fatal.join("\n"));
}

/// Mutated backup ZIP archives (`Store::restore_zip`, the built-in reader): refused or
/// restored, never a panic; a restored store opens to a state the graph had or reports damage.
#[cfg(feature = "persist-zip")]
#[test]
fn mutated_backup_zips_never_crash_or_lie() {
    let fixture = fixture();
    let mut zip = std::io::Cursor::new(Vec::new());
    Store::backup_zip_dir(&fixture.dir, &mut zip).unwrap();
    let zip = zip.into_inner();
    let mut rng = Rng::new(0x21B0_0C1E);
    let mut fatal = Vec::new();
    let mut restored = 0;
    for case in 0..budget(150) {
        let input = mutate_bytes(&mut rng, &zip);
        let states = fixture.states.clone();
        let result = guarded(WATCHDOG, move || -> Result<bool, String> {
            let dir = MemDir::new();
            if Store::restore_zip(input.as_slice(), &dir).is_err() {
                return Ok(false);
            }
            if let Ok(store) = Store::open(&dir, StoreOptions::new())
                && store.maintenance().is_none()
                && !states.contains(&dump(&store.graph().read()))
            {
                return Err("a restored archive opened with a state the graph never had".into());
            }
            Ok(true)
        });
        match result {
            Ok(Ok(true)) => restored += 1,
            Ok(Ok(false)) => {}
            Ok(Err(lie)) => fatal.push(format!("case {case}: {lie}")),
            Err(outcome) => fatal.push(format!("case {case}: {outcome:?}")),
        }
    }
    assert!(fatal.is_empty(), "{}", fatal.join("\n"));
    assert!(restored > 0, "no mutated archive restored at all");
}

/// A flipped bit (it keeps every length) in any file of the store is never silent: the
/// store opens with its final state, in maintenance mode, or not at all. Lengths change only
/// through cuts, and a cut at the end of the last WAL segment is a torn tail by definition
/// (format spec 9.6), so the random test above allows those to lose the cut commits.
#[test]
fn a_flipped_bit_in_any_store_file_is_never_silent() {
    let fixture = fixture();
    let mut silent = Vec::new();
    let mut opens = 0;
    for name in fixture.dir.file_names() {
        let len = fixture.dir.file(&name).unwrap().len();
        let stride = (len / 150).max(1);
        for at in (0..len).step_by(stride) {
            let copy = fixture.dir.deep_copy();
            let mut bytes = copy.file(&name).unwrap();
            bytes[at] ^= 1 << (at % 8);
            copy.put_file(&name, bytes);
            opens += 1;
            if let Ok(store) = Store::open(&copy, StoreOptions::new())
                && store.maintenance().is_none()
                && dump(&store.graph().read()) != fixture.final_state
            {
                silent.push(format!("{name} byte {at}"));
            }
        }
    }
    assert!(opens > 1000, "{opens}");
    assert!(silent.is_empty(), "silently changed data: {silent:?}");
}
