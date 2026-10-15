# Release 0.1.1

A patch release of the 0.1 line. It changes no format and no API; valid stores, snapshots and
journals of 0.1.0 are read and written exactly as before.

## Fixed

- **Files of a newer Graphersal are refused, never taken for damage**
  ([#6](https://github.com/graphersal/graphersal/issues/6)). 0.1.0 refused a store whose `GRAPH`
  file has a newer store format version only when it opened it; `graphersal store info` read the
  snapshot manifests first and called such a store corrupt, `store verify` listed `DAMAGE`, and a
  store whose snapshot or WAL files carried a newer file format version opened in maintenance
  mode and offered a repair that would have lost data. Now every operation (open, read-only
  open, `info`, `verify`, the damage report, repair, fork, checkpoint, backup into or from it,
  restore incl. ZIP, prune, convert, the attic commands, listings and exports; the CLI, the dev
  server and Python alike) refuses a store, backup or single-file store with a newer `GRAPH`
  (either copy), a newer snapshot manifest, snapshot segment or WAL segment anywhere in it (the
  attic too), or a newer single-file container, with `PersistError::UnsupportedVersion` ("written
  by a newer Graphersal"), before it takes a lock, reads data or writes anything. A packed
  snapshot or journal stream of a newer file format version is refused the same way. The rule is
  in the [format specification, section 15](../persistence/format.md#15-versioning) and on
  [Format Versions](../persistence/store_versions.md).
