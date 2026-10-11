//! The checks of a backup on crafted files (format spec 12.6): what a store's own writers never
//! produce but a damaged disk or a foreign file can (a manifest whose two fixed copies are both
//! valid and differ, a WAL segment under another name or of another lineage, a commit before its
//! segment's first, an unreadable file), each a problem with its file and offset; the redundant
//! copies a warning in the store and damage in a copy.

use std::sync::Arc;

use super::backup_check::{
    Findings, check_continuity, check_graph_files, check_marks, check_segment, check_snapshot,
};
use super::files::{DirPath, GRAPH_COPY, GRAPH_FILE, read_graph_file};
use super::*;
use crate::persist::MemDir;
use crate::persist::manifest::FIXED_BYTES;

fn root(dir: &MemDir) -> DirPath {
    DirPath::root(Arc::new(dir.clone()))
}

/// A closed store with two snapshots and WAL segments after them.
fn store() -> MemDir {
    let dir = MemDir::new();
    let options = StoreOptions::new()
        .with_wal_segment_bytes(150)
        .with_checkpoint_after_bytes(0);
    let store = Store::create(&dir, options).unwrap();
    for round in 0..12 {
        store
            .graph()
            .write()
            .traversal_mut()
            .add_v("person")
            .property("name", format!("p{round}"))
            .to_list()
            .unwrap();
        if round == 2 {
            store.checkpoint(None).unwrap();
        }
    }
    store.close().unwrap();
    dir
}

fn snapshots(dir: &MemDir) -> Vec<String> {
    let mut names: Vec<String> = dir
        .file_names()
        .into_iter()
        .filter(|n| n.starts_with("snapshots/") && n.ends_with("/manifest"))
        .map(|n| n.trim_end_matches("/manifest").to_string())
        .collect();
    names.sort();
    names
}

fn wal(dir: &MemDir) -> Vec<(u64, String)> {
    let mut out: Vec<(u64, String)> = dir
        .file_names()
        .into_iter()
        .filter_map(|n| {
            let first = n.strip_prefix("wal/")?.strip_suffix(".wal")?.parse().ok()?;
            Some((first, n))
        })
        .collect();
    out.sort();
    out
}

#[test]
fn a_manifest_fixed_copy_is_healed_in_the_store_and_damage_in_a_copy() {
    let dir = store();
    let newest = snapshots(&dir).pop().unwrap();
    let name = format!("{newest}/manifest");
    let bytes = dir.file(&name).unwrap();
    let tail_at = bytes.len() - FIXED_BYTES;
    for (label, at) in [("head", 40), ("tail", tail_at + 40)] {
        let image = dir.deep_copy();
        let mut damaged = bytes.clone();
        damaged[at] ^= 1;
        image.put_file(&name, damaged);
        let snapshot = root(&image).join(&newest);
        // In the store: a warning, and the manifest with the intact copy in both places.
        let mut findings = Findings::default();
        let healed = check_snapshot(&root(&image), &snapshot, true, &mut findings);
        assert!(findings.is_ok(), "{label}: {:?}", findings.problems);
        assert_eq!(findings.warnings.len(), 1, "{label}");
        assert_eq!(healed.as_deref(), Some(bytes.as_slice()), "{label}");
        // In a copy: damage, with the copy and its offset.
        let mut strict = Findings::default();
        assert!(check_snapshot(&root(&image), &snapshot, false, &mut strict).is_none());
        assert_eq!(strict.problems.len(), 1, "{label}");
        assert!(
            strict.problems[0].reason.starts_with(label),
            "{:?}",
            strict.problems
        );
    }
    // Both copies valid and different (the older snapshot's fixed part as the tail): damage in
    // the store too, never healed.
    let older = snapshots(&dir)[0].clone();
    let other = dir.file(&format!("{older}/manifest")).unwrap();
    let mut mixed = bytes.clone();
    mixed[tail_at..].copy_from_slice(&other[..FIXED_BYTES]);
    let image = dir.deep_copy();
    image.put_file(&name, mixed);
    let mut findings = Findings::default();
    let healed = check_snapshot(
        &root(&image),
        &root(&image).join(&newest),
        true,
        &mut findings,
    );
    assert!(healed.is_none());
    assert!(
        findings.problems[0].reason.contains("differ"),
        "{:?}",
        findings.problems
    );
    // A missing segment file: the snapshot's damage, named.
    let image = dir.deep_copy();
    let segment = image
        .file_names()
        .into_iter()
        .find(|n| n.starts_with(&newest) && n.ends_with(".seg"))
        .unwrap();
    let kept = MemDir::new();
    for (n, b) in super::merge_tests::listing(&image) {
        if n != segment {
            kept.put_file(&n, b);
        }
    }
    let mut findings = Findings::default();
    check_snapshot(
        &root(&kept),
        &root(&kept).join(&newest),
        true,
        &mut findings,
    );
    assert_eq!(findings.problems.len(), 1);
}

#[test]
fn wal_segments_under_another_name_of_another_lineage_or_with_early_commits_are_damage() {
    let dir = store();
    let graph = read_graph_file(&root(&dir)).unwrap().file;
    let segments = wal(&dir);
    assert!(segments.len() >= 3, "{segments:?}");
    let (first, name) = segments[1].clone();
    let path = root(&dir).join(&name);
    // Intact.
    let mut findings = Findings::default();
    let check = check_segment(
        &root(&dir),
        first,
        &path,
        None,
        false,
        Some(&graph),
        &mut findings,
    )
    .unwrap();
    assert!(findings.is_ok() && !check.commits.is_empty());
    // Another segment's bytes under this name: the header names another first commit, and the
    // commits lie before this name.
    let (later, _) = segments[2].clone();
    let mut findings = Findings::default();
    assert!(
        check_segment(
            &root(&dir),
            later,
            &path,
            None,
            false,
            Some(&graph),
            &mut findings
        )
        .is_none()
    );
    let reasons: Vec<&str> = findings
        .problems
        .iter()
        .map(|p| p.reason.as_str())
        .collect();
    assert!(
        reasons
            .iter()
            .any(|r| r.contains("its header says first commit")),
        "{reasons:?}"
    );
    assert!(
        reasons
            .iter()
            .any(|r| r.contains("before the segment's first commit")),
        "{reasons:?}"
    );
    // A lineage the chain does not hold.
    let mut foreign = graph.clone();
    foreign.lineage[0].graph_id ^= 1;
    foreign.graph_id ^= 1;
    let mut findings = Findings::default();
    assert!(
        check_segment(
            &root(&dir),
            first,
            &path,
            None,
            false,
            Some(&foreign),
            &mut findings
        )
        .is_none()
    );
    assert!(
        findings.problems[0].reason.contains("lineage"),
        "{:?}",
        findings.problems
    );
    // A missing file, and one shorter than its header.
    let mut findings = Findings::default();
    let missing = root(&dir).join("wal/00000000000000099999.wal");
    assert!(
        check_segment(
            &root(&dir),
            99_999,
            &missing,
            None,
            false,
            None,
            &mut findings
        )
        .is_none()
    );
    assert_eq!(findings.problems.len(), 1);
    let image = dir.deep_copy();
    image.put_file(&name, dir.file(&name).unwrap()[..10].to_vec());
    let mut findings = Findings::default();
    let short = root(&image).join(&name);
    assert!(
        check_segment(
            &root(&image),
            first,
            &short,
            None,
            false,
            None,
            &mut findings
        )
        .is_none()
    );
    assert!(
        findings.problems[0].reason.contains("truncated"),
        "{:?}",
        findings.problems
    );
    // A torn last record where none may be: damage; where it may: the end before it.
    let mut torn = dir.file(&name).unwrap();
    let complete = torn.len() as u64;
    torn.extend_from_slice(&[0x57, 0x53, 0x52]);
    image.put_file(&name, torn);
    let mut findings = Findings::default();
    assert!(
        check_segment(
            &root(&image),
            first,
            &short,
            None,
            false,
            None,
            &mut findings
        )
        .is_none()
    );
    let mut findings = Findings::default();
    let check = check_segment(
        &root(&image),
        first,
        &short,
        None,
        true,
        None,
        &mut findings,
    )
    .unwrap();
    assert_eq!(check.end, complete);
}

#[test]
fn a_gap_in_the_copied_commits_is_damage() {
    let dir = store();
    let graph = read_graph_file(&root(&dir)).unwrap().file;
    let segments = wal(&dir);
    let mut checks = Vec::new();
    for (first, name) in &segments {
        let mut findings = Findings::default();
        let path = root(&dir).join(name);
        checks.push((
            *first,
            check_segment(
                &root(&dir),
                *first,
                &path,
                None,
                false,
                Some(&graph),
                &mut findings,
            )
            .unwrap(),
        ));
    }
    let all: Vec<(u64, &super::backup_check::SegmentCheck)> =
        checks.iter().map(|(f, c)| (*f, c)).collect();
    let mut findings = Findings::default();
    check_continuity(&all, Some(0), &mut findings);
    assert!(findings.is_ok(), "{:?}", findings.problems);
    // A segment left out: its commits are missing.
    let gap: Vec<(u64, &super::backup_check::SegmentCheck)> = all
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, c)| *c)
        .collect();
    let mut findings = Findings::default();
    check_continuity(&gap, Some(0), &mut findings);
    assert_eq!(findings.problems.len(), 1, "{:?}", findings.problems);
    // The first commit after the snapshot must be its successor.
    let mut findings = Findings::default();
    check_continuity(&all[1..], Some(0), &mut findings);
    assert_eq!(findings.problems.len(), 1, "{:?}", findings.problems);
}

#[test]
fn graph_files_and_marks_damage_and_warnings() {
    let dir = store();
    // Both GRAPH copies unusable: damage, both named.
    let image = dir.deep_copy();
    for name in [GRAPH_FILE, GRAPH_COPY] {
        let mut bytes = image.file(name).unwrap();
        bytes[20] ^= 1;
        image.put_file(name, bytes);
    }
    let mut findings = Findings::default();
    assert!(
        check_graph_files(&root(&image), &mut findings)
            .unwrap()
            .is_none()
    );
    assert_eq!(findings.problems.len(), 2);
    // Both valid, `GRAPH` with the longer lineage (a rewrite interrupted after it): it is used.
    let image = dir.deep_copy();
    let mut longer = read_graph_file(&root(&image)).unwrap().file;
    let mut entry = longer.lineage[0];
    entry.branched_at = 3;
    entry.graph_id ^= 7;
    longer.lineage.insert(0, entry);
    longer.graph_id = entry.graph_id;
    image.put_file(GRAPH_FILE, longer.encode());
    let mut findings = Findings::default();
    let used = check_graph_files(&root(&image), &mut findings)
        .unwrap()
        .unwrap();
    assert_eq!(used.lineage.len(), 2);
    // One: a warning, the other copy used.
    let image = dir.deep_copy();
    image.put_file(GRAPH_COPY, b"garbage".to_vec());
    let mut findings = Findings::default();
    assert!(
        check_graph_files(&root(&image), &mut findings)
            .unwrap()
            .is_some()
    );
    assert!(findings.is_ok() && findings.warnings.len() == 1);
    // A marks file that cannot be read (here: a directory) or is damaged: a warning.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let temp = std::env::temp_dir().join(format!("bk1-marks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(temp.join("marks")).unwrap();
        let disk = DirPath::root(crate::persist::store_dir_at(&temp));
        let mut findings = Findings::default();
        assert!(check_marks(&disk, &mut findings).is_none());
        assert!(
            findings.is_ok() && findings.warnings.len() == 1,
            "{findings:?}"
        );
        std::fs::remove_dir_all(&temp).unwrap();
    }
    let image = dir.deep_copy();
    image.put_file("marks", vec![9, 9, 9]);
    let mut findings = Findings::default();
    assert!(check_marks(&root(&image), &mut findings).is_none());
    assert_eq!(findings.warnings.len(), 1);
}
