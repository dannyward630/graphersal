# Streaming checkpoints and verified backups: design

Status: IMPLEMENTED in 0.1.0: the streaming encoder (step CP1), the merge checkpoint (step CP2), verified
backups, backup from memory and the damage policy. The normative byte layout is the format spec
(`book/src/persistence/format.md`, sections 7, 12.3, 12.6, 12.11); the parameters fixed at creation are
listed on `book/src/persistence/creation_parameters.md`. When this design and the code disagree, the code
wins.

## 1. Starting point

- `Store::checkpoint` takes the live graph's READ LOCK and encodes the WHOLE snapshot into memory
  (`encode_sections` -> `SnapshotSections` with every segment's bytes), then writes it. Commits wait for the
  whole encode; memory grows by the encoded snapshot plus a sorted id index. This contradicts the persistence
  design principle "a compactor must not need 2x memory", for which the format was designed (id-sorted segments, self-contained WAL records, edge endpoints as ids, immutable
  segments with id ranges, incremental snapshots reusing segments).
- A full backup of an open store copies the WAL by length up to the durable end without reading the records'
  CRCs; `marks` is copied as is. Snapshots are verified before and after; increments re-read the appended WAL.

The decisions below replace both.

## 2. Decisions

1. **Streaming snapshot encoder.** Encode and write chunk by chunk straight into the snapshot's `tmp-*`
   files; the manifest last (CRCs and chunk index collected on the way). Memory: one chunk, its string table,
   the sorted id index (about 24 bytes per element). Used where a snapshot must come from the live graph: the
   first snapshot of a store (no base), `export_packed` / `.gsnap`, a fork from memory, `backup --from-memory`.
   *Made precise in CP1:* a segment file starts with its chunk count and footer offset (format spec 7.1) and a
   packed stream starts with the manifest, while `StoreDir` files are only appended (never patched) and a stream
   cannot seek; so the encoder runs TWO passes over the id-sorted elements: a plan pass encodes every chunk and
   keeps only its offset, ids and CRC, an emit pass encodes each chunk again from its element range and writes it
   (checked against the plan). Memory as above; the price is twice the encoding CPU under the read lock. The
   CP2 merge has the same constraint for the segments it rewrites (plan the merge, then merge again to write).
2. **Checkpoint = merge of the previous snapshot with the WAL, no live graph, no lock.** WAL changes since the
   base snapshot are collected by id; segments whose id ranges are untouched are reused by reference
   (incremental snapshot, manifest flag bit 0); touched segments are read as a stream and merged with the
   changes (merge on the sorted id) into new segments. Memory proportional to the DELTA (the WAL since the
   last checkpoint), not to the graph; IO only for touched segments. Runs as the automatic checkpoint (a
   background thread natively; `run_due_checkpoint` on wasm) and as a console command on a closed store
   (`graphersal store checkpoint <dir>`), and through the live `Store::checkpoint`.
3. **The merge verifies every checksum it reads** (manifest, segment and chunk CRCs, WAL header and body CRCs,
   commit continuity) and stops on damage with a report: a damaged input never yields a snapshot. The new
   snapshot is verified where it landed before the rename.
4. **Backups verify every checksum** of what they copy, in the source and again in the copy: WAL records,
   `marks`, `GRAPH`, snapshots (full backups and increments, directory and ZIP). On damage the backup stops,
   removes a partial new backup / rolls an increment back (existing `INTENT` mechanism), and reports the
   damage like `verify`. The message recommends repairing the MAIN store
   (`graphersal store repair <dir> --to <new_dir>`), never the backup.
5. **`backup --from-memory`, only when the store on disk is damaged** and the store is open in this process
   (its in-memory graph was verified at load and changed only by commits): the streaming encoder writes a
   snapshot of the live graph straight into the BACKUP directory under the read lock (an acceptable price:
   saving the data matters more); `GRAPH` of the backup with the marker, the source's `graph_id` and lineage;
   an empty WAL segment after that commit. NOTHING is written into the source directory (a dying disk must
   not be written to). Works into a new root and as an increment (the snapshot is newer than everything in
   the backup; a later normal increment may then need a full backup, and says so).
6. **Damage found on disk while a store is open** (by a backup, a checkpoint merge, verify) is governed by a
   **damage policy fixed when the store is created**:
   - `maintenance` (the DEFAULT): the open store switches to the maintenance (read-only) mode with the
     reason; commits are refused, reads and `--from-memory` keep working; the playground, the dev server and
     MCP show it;
   - `continue`: the store keeps accepting commits; the damage is reported (store info, the Store menu, a
     warning in the logs) and stays reported until a repair.
   The policy is chosen at creation (`StoreOptions` create option, `graphersal store create --on-damage
   continue`, Python, the dev server's `--create-store`) and can NEVER be changed afterwards: no API, no CLI.
   It is stored in `GRAPH` (flag bit 5 = `continue`; a reader that does not know the bit falls back to the
   safe default `maintenance`). Fork, repair, convert, backup and restore carry it unchanged (a fork is
   the same data under a new lineage, not a new decision). It does not affect damage found at OPEN time: a
   graph that cannot be loaded intact always opens in maintenance mode.
7. **One documentation place for the parameters fixed at creation**: a book page in the Persistence part
   (`persistence/creation_parameters.md`) listing
 every store parameter that is set when the store is
   created and never changes (today: `graph_id` and creation time, the chunk target size, the backend kind's
   container format version; new: the damage policy), each with its default, how to set it (Rust, CLI,
   Python, dev server), where it is stored (format spec section) and what copies carry it; next to it the
   per-open options (`StoreOptions` that may differ on every open) so the two kinds are not confused. Every
   future creation-time parameter is added there (rule in `CLAUDE.md`).

## 3. Non-goals

A fuzzy (ARIES-style) checkpoint of the live graph in batches with idempotent replay: decision 2 makes it
unnecessary.
