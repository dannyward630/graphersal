# Format Versions and Upgrade

Every file of the format starts with a magic and a version. There are two axes (and the
single-file container has its own container version):

- the **store format version**, in the store's `GRAPH` file: this release writes and reads
  **version 2** (`graphersal::persist::STORE_FORMAT_VERSION`); `graphersal store info` prints it
  (`backend  directory (format version 2)`, `format  version 2`);
- the **file format version** of snapshot manifests and segments, WAL segments and packed
  snapshots (`.gsnap`): **version 1** (`graphersal::persist::FORMAT_VERSION`).

| Version | What changed |
|---|---|
| store 1 | the first store format |
| store 2 | the store identity `store_id` in `GRAPH` (offset 100), which incremental backups check |

| The store or file is | Then |
|---|---|
| the current version | read and written as usual |
| **newer** than this build | refused with `PersistError::UnsupportedVersion` ("written by a newer format version"): never guessed at. Use a newer Graphersal |
| a store of an **older** store format version | refused with `PersistError::OlderFormat` by every operation (open, read-only open, `store info`, `verify`, backup, restore, fork, repair, convert); nothing is changed, never reported as damage |

**Migration.** A store of an older store format is moved through a packed snapshot: the build
that wrote it exports the current state (`graphersal --graph old_store/ -e
'g.export_snapshot("graph.gsnap")'`), and this build creates a new store from the file
(`graphersal store create new_store/ --from graph.gsnap`). The file format did not change, so the
packed snapshot of the older build loads as it is. The new store carries the graph, its schema
and its catalog; marks, the attic and backups of the old store are not carried. The full recipe:
[Command Cheat Sheet](../users_guide/command_cheat_sheet.md#migrate-a-store-from-an-older-format).

What counts as a format change, and the compatibility promises for third-party readers and
writers, are in the [format specification, section 15](format.md#15-versioning).

## Growing without a new version

New kinds of definitions in the catalog (saved queries, property indexes and compression rules
today; procedures and others later), new keys of a definition, and new kinds of snapshot files do **not** change the
format version ([format specification, section 16](format.md#16-extensibility)). A build that
meets something it does not know:

| It finds | Then |
|---|---|
| a definition of an unknown kind | kept byte for byte and written back unchanged at every checkpoint, fork, backup and repair |
| an unknown key of a saved query | kept and written back |
| a **critical** definition of an unknown kind (one that must stay consistent with the data, such as an index), or a snapshot file of a critical unknown kind | the store opens **read-only** (`Store::read_only_reason`, `PersistError::ReadOnly`): reading, `verify`, export, fork and backups work; writes need the newer version that wrote it |

Stores written before the catalog existed open unchanged: their catalog is empty, the first
saved query goes into the WAL and the next checkpoint writes it into the snapshot.
