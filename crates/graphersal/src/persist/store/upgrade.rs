//! The store converter (format spec 15.1): upgrades a store, a backup directory or a
//! single-file store of an older RELEASED store format version to the current one, without the
//! Graphersal that wrote it.
//!
//! Every build reads the files of every released format version (15), so an upgrade rewrites
//! nothing but `GRAPH` and `GRAPH.copy`: the snapshots, WAL segments, `marks`, the attic and
//! everything else stay byte for byte (each file keeps its own file format version, which every
//! reader accepts), so the history, the marks, the lineage, the attic entries and every creation
//! parameter (the store id among them) are carried exactly. The journal never appends to a WAL
//! segment of an older file format version: the first read-write open starts a new segment
//! (`attach_journal`).
//!
//! The older store is checked first through an [`UpgradeView`]: the store as this build reads
//! it once upgraded (its `GRAPH` copies served with the current version word, nothing written),
//! scrubbed by the ordinary `verify`; a store that does not verify is refused (repair it into a
//! new current store instead, `repair_dir` reads older stores through the same view).

use std::io::{self, Read};
use std::sync::Arc;

use super::super::codec::{crc, put_u32};
use super::super::dir::{DirEntry, DirLockGuard, EntryKind, LockKind, StoreDir, StoreFile};
use super::super::error::PersistError;
use super::files::{self, DirLock, DirPath, GRAPH_COPY, GRAPH_FILE, GraphFile, STORE_VERSION};
use super::maintenance::{self, VerifyReport};
use super::{BackupMarker, Store};
use crate::persist::StoreFailure;

/// The oldest store format version this build upgrades: 2, the version of the first release
/// (Graphersal 0.1.0). Version 1 was never released (it has no store id); such a store keeps
/// the migration through a packed snapshot (its `OlderFormat` help).
pub(crate) const OLDEST_UPGRADABLE_STORE_VERSION: u32 = 2;

/// The name of the marker a copying upgrade writes into its target first and removes last: a
/// target that still holds it is an interrupted upgrade (no `GRAPH` yet, or written but the
/// marker not yet removed), which the next upgrade into it clears and repeats. A `.tmp` name:
/// a copy never carries it and an open removes it.
const UPGRADE_MARKER: &str = "upgrade-in-progress.tmp";

/// What an upgrade did ([`Store::upgrade`], [`Store::upgrade_in_place`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct UpgradeReport {
    /// The store format version the store had (the current one when it needed no upgrade).
    pub from_version: u32,
    /// The store format version it has now ([`STORE_FORMAT_VERSION`](crate::persist::STORE_FORMAT_VERSION)).
    pub to_version: u32,
    /// The store was already of the current version: nothing was written (in place only; a
    /// copying upgrade refuses such a store).
    pub already_current: bool,
    /// Whether the store was upgraded in place (`false`: into a new copy).
    pub in_place: bool,
    /// The backup marker, when the upgraded directory is a backup (it stays one).
    pub backup: Option<BackupMarker>,
    /// Files and bytes written into the copy (`0` in place: only `GRAPH` and `GRAPH.copy`
    /// are rewritten).
    pub files_copied: usize,
    /// See [`Self::files_copied`].
    pub bytes_copied: u64,
    /// The scrub of the older store before anything was written (every checksum of every
    /// snapshot, WAL record, `GRAPH` copy, attic entry): a store with problems is refused.
    pub verify: VerifyReport,
}

/// What one `GRAPH` copy is, for the converter.
enum GraphCopy {
    /// A valid `GRAPH` of an older version (`older_version`).
    Older(u32),
    /// Current, newer, missing, damaged: left to the reader's own diagnosis.
    Other,
}

fn classify(dir: &DirPath, name: &str) -> (GraphCopy, Option<Vec<u8>>) {
    let Ok(bytes) = files::read_bounded(&dir.join(name), 64 << 20) else {
        return (GraphCopy::Other, None);
    };
    let kind = if let Some(version) = GraphFile::older_version(&bytes) {
        GraphCopy::Older(version)
    } else {
        GraphCopy::Other
    };
    (kind, Some(bytes))
}

/// The `GRAPH` bytes of an upgradable older version with the current version word (and its
/// checksum); everything else of the file is kept. `None`: not an upgradable older `GRAPH`.
fn patched(bytes: &[u8]) -> Option<Vec<u8>> {
    let version = GraphFile::older_version(bytes)?;
    if version < OLDEST_UPGRADABLE_STORE_VERSION {
        return None;
    }
    // Versions 2 and 3 have the same layout (format spec 5.1, 15): only the version word moves.
    let mut out = bytes.to_vec();
    let fixed = files::GRAPH_FIXED_BYTES;
    out[8..12].copy_from_slice(&STORE_VERSION.to_le_bytes());
    let mut checksum = Vec::with_capacity(4);
    put_u32(&mut checksum, crc(&out[..fixed - 4]));
    out[fixed - 4..fixed].copy_from_slice(&checksum);
    Some(out)
}

/// The store in `dir` as this build reads it once upgraded, when it is of an upgradable older
/// version (`None`: it is not; the caller keeps its own handling): a read-only view whose
/// `GRAPH` copies carry the current version.
pub(crate) fn older_view(dir: &DirPath) -> Option<DirPath> {
    let mut graph = Vec::new();
    for name in [GRAPH_FILE, GRAPH_COPY] {
        if let (GraphCopy::Older(_), Some(bytes)) = classify(dir, name)
            && let Some(bytes) = patched(&bytes)
        {
            graph.push((name.to_string(), bytes));
        }
    }
    (!graph.is_empty()).then(|| {
        DirPath::root(Arc::new(UpgradeView {
            inner: dir.store_dir().clone(),
            graph,
        }))
    })
}

/// The version found, or the refusal of a store this converter does not take.
fn source_version(dir: &DirPath) -> Result<u32, PersistError> {
    if !dir.join(GRAPH_FILE).exists() && !dir.join(GRAPH_COPY).exists() {
        return Err(PersistError::NotAStore {
            path: dir.display(),
            reason: "it has no GRAPH file".into(),
        });
    }
    let (main, _) = classify(dir, GRAPH_FILE);
    let (copy, _) = classify(dir, GRAPH_COPY);
    let older = |kind: &GraphCopy| match kind {
        GraphCopy::Older(version) => Some(*version),
        _ => None,
    };
    let upgradable = [older(&main), older(&copy)]
        .into_iter()
        .flatten()
        .filter(|version| *version >= OLDEST_UPGRADABLE_STORE_VERSION)
        .max();
    match upgradable {
        Some(version) => Ok(version),
        // The current version, a newer one, a pre-release one, damage: the reader says which.
        None => {
            let read = files::read_graph_file(dir)?;
            super::guard_version(dir, read.file.version)?;
            Ok(read.file.version)
        }
    }
}

/// Scrubs the older store through its upgraded view; problems refuse the upgrade.
fn verify_source(view: &DirPath, source: &DirPath) -> Result<VerifyReport, PersistError> {
    let report = maintenance::verify(view, None)?;
    if !report.is_ok() {
        let first: Vec<String> = report
            .problems
            .iter()
            .take(5)
            .map(|p| format!("{}: {}", p.path, p.reason))
            .collect();
        return Err(PersistError::store(
            "upgrade the store",
            StoreFailure::DamagedSource,
            format!(
                "{} does not verify ({} problem(s): {}{})",
                source.display(),
                report.problems.len(),
                first.join("; "),
                if report.problems.len() > first.len() {
                    "; ..."
                } else {
                    ""
                }
            ),
        ));
    }
    Ok(report)
}

/// [`Store::upgrade_in_place`].
pub(super) fn upgrade_in_place(dir: &DirPath) -> Result<UpgradeReport, PersistError> {
    let _lock = DirLock::exclusive(dir)?;
    let from_version = source_version(dir)?;
    if from_version == STORE_VERSION {
        return Ok(UpgradeReport {
            from_version,
            to_version: STORE_VERSION,
            already_current: true,
            in_place: true,
            backup: files::read_graph_file(dir)?.file.backup,
            ..UpgradeReport::default()
        });
    }
    let view = older_view(dir).ok_or_else(|| {
        PersistError::corrupt(dir.display(), 0, "the GRAPH file changed while it was read")
    })?;
    let verify = verify_source(&view, dir)?;
    // The one change: GRAPH.copy, then GRAPH, each replaced atomically (format spec 5.3). Every
    // state a crash can leave is readable: both old (the store is unchanged), the copy new and
    // GRAPH old (a reader of this version uses the copy and rewrites GRAPH at its next write,
    // a repeated upgrade does it at once), both new (done). No other file changes.
    let file = files::read_graph_file(&view)?.file;
    files::write_graph_file(dir, &file)?;
    Ok(UpgradeReport {
        from_version,
        to_version: STORE_VERSION,
        already_current: false,
        in_place: true,
        backup: file.backup,
        files_copied: 0,
        bytes_copied: 0,
        verify,
    })
}

/// [`Store::upgrade`].
pub(super) fn upgrade_copy(from: &DirPath, to: &DirPath) -> Result<UpgradeReport, PersistError> {
    // The writer lock keeps a writer of the older version (or this one) out while the files
    // are copied; readers take no lock. Nothing of `from` is written.
    let _lock = DirLock::exclusive(from)?;
    let from_version = source_version(from)?;
    if from_version == STORE_VERSION {
        return Err(PersistError::store(
            "upgrade the store",
            StoreFailure::AlreadyCurrent,
            format!(
                "{} already has the current store format version {STORE_VERSION}",
                from.display()
            ),
        ));
    }
    let view = older_view(from).ok_or_else(|| {
        PersistError::corrupt(
            from.display(),
            0,
            "the GRAPH file changed while it was read",
        )
    })?;
    let verify = verify_source(&view, from)?;
    let file = files::read_graph_file(&view)?.file;

    // An interrupted upgrade into `to` is cleared and repeated; anything else in it refuses.
    if to.join(UPGRADE_MARKER).exists() {
        files::undo_new_dir(to, true);
    }
    if files::has_entries(to) {
        return Err(PersistError::DirectoryNotEmpty { path: to.display() });
    }
    let existed = to.exists();
    let result = (|| {
        to.create_dir_all()
            .map_err(|err| PersistError::io("creating the upgraded store", err))?;
        files::write_atomic(to, UPGRADE_MARKER, from.display().as_bytes())?;
        let mut copied = (0usize, 0u64);
        let mut graph_files = Vec::new();
        super::copy_tree(from, to, from, &mut copied, &mut graph_files)?;
        // The copy is checked before it becomes a store: a view of it with the new GRAPH
        // copies in memory. Until they are written it has no GRAPH, so an interrupted upgrade
        // never leaves something that opens as a store.
        let bytes = file.encode();
        let staged = DirPath::root(Arc::new(UpgradeView {
            inner: to.store_dir().clone(),
            graph: vec![
                (GRAPH_FILE.to_string(), bytes.clone()),
                (GRAPH_COPY.to_string(), bytes.clone()),
            ],
        }));
        let checked = maintenance::verify(&staged, None)?;
        if !checked.is_ok() {
            return Err(PersistError::store(
                "upgrade the store",
                StoreFailure::CopyDoesNotVerify,
                format!(
                    "the copy in {} does not verify: {:?}",
                    to.display(),
                    checked.problems
                ),
            ));
        }
        files::write_graph_file(to, &file)?;
        copied.0 += 2;
        copied.1 += 2 * bytes.len() as u64;
        to.join(UPGRADE_MARKER)
            .remove_file()
            .map_err(|err| PersistError::io("finishing the upgraded store", err))?;
        files::sync_dir(to)?;
        Ok(copied)
    })();
    match result {
        Ok((files_copied, bytes_copied)) => Ok(UpgradeReport {
            from_version,
            to_version: STORE_VERSION,
            already_current: false,
            in_place: false,
            backup: file.backup,
            files_copied,
            bytes_copied,
            verify,
        }),
        Err(err) => {
            files::undo_new_dir(to, existed);
            Err(err)
        }
    }
}

impl Store {
    /// Upgrades the store (or backup directory, or single-file store) in `from`, written by an
    /// older RELEASED Graphersal (store format version 2, Graphersal 0.1.x), into a NEW store
    /// in `to` (a new or empty directory, or a new single file such as `x.gstore`) of the
    /// current format, without the older Graphersal (CLI: `graphersal store upgrade <from>
    /// <to>`). Nothing of `from` changes; its writer lock is held while it is copied (a store
    /// still open by a writer is refused with [`PersistError::Locked`]).
    ///
    /// Everything is carried exactly: the graph, its schema and catalog, the whole history
    /// (snapshots and WAL), marks, lineage, attic entries (they stay restorable) and every
    /// creation parameter, the store id included: the copy is the SAME store in the new format
    /// (keep using it, not the older one; an upgraded backup of it continues to take
    /// increments). Only `GRAPH` and `GRAPH.copy` are rewritten; every other file is copied
    /// byte for byte and stays readable as it is (format spec 15.1). A backup directory stays
    /// a backup.
    ///
    /// The older store is scrubbed first (every checksum); damage is refused with
    /// [`StoreFailure::DamagedSource`] (repair it into a new current store instead:
    /// [`Store::repair_dir`] reads older stores). The copy is verified before its `GRAPH` is
    /// written, so an interrupted upgrade never leaves a store in `to` (the next upgrade into
    /// `to` clears what it left). A store of the current format is refused with
    /// [`StoreFailure::AlreadyCurrent`].
    pub fn upgrade(
        from: impl super::IntoStoreDir,
        to: impl super::IntoStoreDir,
    ) -> Result<UpgradeReport, PersistError> {
        upgrade_copy(
            &DirPath::root(from.into_store_dir()),
            &DirPath::root(to.into_store_dir()),
        )
    }

    /// Upgrades the store (or backup directory, or single-file store) in `dir`, written by an
    /// older RELEASED Graphersal, IN PLACE to the current format (CLI: `graphersal store
    /// upgrade <dir> --in-place`). Carries everything like [`Store::upgrade`]; takes the
    /// writer lock; the store is scrubbed first and refused when damaged, before anything is
    /// written. Then only `GRAPH.copy` and `GRAPH` are replaced (each atomically), so a crash
    /// at any point leaves the store readable, older or upgraded, never losing anything; run
    /// it again to complete it. Afterwards a Graphersal of the older version no longer opens
    /// the store (it refuses it as newer): back it up first if you might go back. A store of
    /// the current format is left as it is ([`UpgradeReport::already_current`]).
    pub fn upgrade_in_place(dir: impl super::IntoStoreDir) -> Result<UpgradeReport, PersistError> {
        upgrade_in_place(&DirPath::root(dir.into_store_dir()))
    }
}

/// A read-only view of a store directory whose `GRAPH` copies are served from memory (the
/// upgraded bytes of an older store, or the not yet written ones of an upgraded copy); every
/// other name reads through. Writes are refused: the view only reads (`verify`, a salvage).
#[derive(Debug)]
pub(crate) struct UpgradeView {
    inner: Arc<dyn StoreDir>,
    graph: Vec<(String, Vec<u8>)>,
}

impl UpgradeView {
    fn served(&self, name: &str) -> Option<&[u8]> {
        self.graph
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, bytes)| bytes.as_slice())
    }

    fn refuse() -> io::Error {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "a store being upgraded is only read",
        )
    }
}

impl StoreDir for UpgradeView {
    fn location(&self) -> String {
        self.inner.location()
    }

    fn is_volatile(&self) -> bool {
        self.inner.is_volatile()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn local_path(&self) -> Option<&std::path::Path> {
        self.inner.local_path()
    }

    fn entry_kind(&self, name: &str) -> io::Result<Option<EntryKind>> {
        if self.served(name).is_some() {
            return Ok(Some(EntryKind::File));
        }
        self.inner.entry_kind(name)
    }

    fn list(&self, dir: &str) -> io::Result<Vec<DirEntry>> {
        let mut entries = self.inner.list(dir)?;
        if dir.is_empty() {
            for (name, _) in &self.graph {
                if !entries.iter().any(|e| &e.name == name) {
                    entries.push(DirEntry::new(name.clone(), EntryKind::File));
                }
            }
        }
        Ok(entries)
    }

    fn file_len(&self, name: &str) -> io::Result<u64> {
        match self.served(name) {
            Some(bytes) => Ok(bytes.len() as u64),
            None => self.inner.file_len(name),
        }
    }

    fn read(&self, name: &str, max: u64) -> io::Result<Vec<u8>> {
        match self.served(name) {
            Some(bytes) if bytes.len() as u64 > max => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{name} is larger than {max} bytes"),
            )),
            Some(bytes) => Ok(bytes.to_vec()),
            None => self.inner.read(name, max),
        }
    }

    fn read_range(&self, name: &str, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        match self.served(name) {
            Some(bytes) => usize::try_from(offset)
                .ok()
                .and_then(|start| bytes.get(start..start.checked_add(len)?))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof)),
            None => self.inner.read_range(name, offset, len),
        }
    }

    fn open_read(&self, name: &str) -> io::Result<Box<dyn Read + Send>> {
        match self.served(name) {
            Some(bytes) => Ok(Box::new(io::Cursor::new(bytes.to_vec()))),
            None => self.inner.open_read(name),
        }
    }

    fn create_dir_all(&self, _: &str) -> io::Result<()> {
        Err(Self::refuse())
    }

    fn create(&self, _: &str, _: bool) -> io::Result<Box<dyn StoreFile>> {
        Err(Self::refuse())
    }

    fn open_write(&self, _: &str, _: u64) -> io::Result<Box<dyn StoreFile>> {
        Err(Self::refuse())
    }

    fn append(&self, _: &str, _: &[u8]) -> io::Result<()> {
        Err(Self::refuse())
    }

    fn write_atomic(&self, _: &str, _: &[u8]) -> io::Result<()> {
        Err(Self::refuse())
    }

    fn rename(&self, _: &str, _: &str) -> io::Result<()> {
        Err(Self::refuse())
    }

    fn truncate(&self, _: &str, _: u64) -> io::Result<()> {
        Err(Self::refuse())
    }

    fn remove_file(&self, _: &str) -> io::Result<()> {
        Err(Self::refuse())
    }

    fn remove_tree(&self, _: &str) -> io::Result<()> {
        Err(Self::refuse())
    }

    fn sync_dir(&self, _: &str) -> io::Result<()> {
        Ok(())
    }

    fn lock(&self, kind: LockKind) -> io::Result<Option<DirLockGuard>> {
        self.inner.lock(kind)
    }

    fn check(&self) -> super::super::dir::DirCheck {
        self.inner.check()
    }
}
