# Marks

A **mark** names a position in the store's history: "the state right after the last commit".
It is a record in the WAL (and a line in the `marks` file) and costs a few bytes; no data is
copied. Use it to remember a moment by name, "before-import", and come back to it later with
`--at-mark before-import`.

```rust,no_run
# use graphersal::persist::{Store, StoreOptions};
# let store = Store::open("data", StoreOptions::new())?;
let mark = store.mark("before-import")?;
println!("{} at commit {}", mark.name(), mark.commit_seq());
for mark in store.marks()? {
    println!("{} {} {}", mark.commit_seq(), mark.time(), mark.name());
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

```text
$ graphersal store mark data/ before-import
Mark "before-import" at commit 1.
$ graphersal store marks data/
           1  2026-10-08T07:05:08Z  before-import
```

- Mark names are **unique along the store's whole history**, across rollbacks and lineages; a
  duplicate is refused. A mark is never removed (an in-place rollback hides the marks after its
  target in the attic with the rest of that history).
- A mark needs its history: a snapshot at or before it and the WAL from there to it. A
  [prune](store_prune.md#prune-and-marks) that removes that history makes the mark
  **unreachable**: `store.marks()`, `store info` and the Store menu leave it out from then on,
  `--at-mark` on it fails with "a prune removed the history it needs", and its name stays taken
  (setting it again says so). The prune names such marks (`report.marks_unreachable`), and the
  compaction advice names them before you compact.
- A mark's time is the time it was set (never earlier than the last commit).
- From a script: `g.mark("before-import")` on a graph that has a journal (the CLI's `--graph
  data/`, the dev server; it needs the `Update` permission on data). From the dev server: the
  Store menu's **Mark** (MCP has no persistence tools). From Python: `store.mark(name)`.
- A mark inside an open transaction, or on a graph without a journal, is an error
  (`GraphError::MarkUnavailable`). A store that is a backup, read-only or in maintenance mode
  refuses marks.
- Marks are not copied by a fork or a repair: they belong to the history they were set in.

## Snapshots and marks

Both name a point in the store's history, but they are different things, and neither limits
where you can go back to: **every commit is a target** (`--at-commit N`), and so is every point
in time (`--at-time`).

| | Snapshot | Mark |
|---|---|---|
| What it is | a full copy of the graph at a commit, in `snapshots/<commit>/` | a name for a position in the history (a WAL record and a line in `marks`) |
| Made by | `store.checkpoint(name)`, `store checkpoint`, the Store menu's Checkpoint, automatically past 256 MiB of WAL | `store.mark(name)`, `g.mark(name)`, `store mark`, the Store menu's Mark |
| Costs | the size of the graph on disk; writing it [merges](store_checkpoints.md#how-a-checkpoint-is-written) the previous snapshot with the WAL after it (memory for the changes only) | a few bytes |
| Good for | a fast open and a fast load of that state (only the WAL after it is replayed); a `.gsnap` download | remembering "before the import" by name |
| Removed by | `prune` (the last two always stay) | never; a prune of the history before it makes it unreachable |
