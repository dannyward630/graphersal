# Point in Time

Every state the retained history covers can be brought back: read-only, as a new store, or as the
store's own state again.

## Targets

| Target | Rust (`RecoveryTarget`) | CLI | Means |
|---|---|---|---|
| the end | `Latest` | (no option) | the current state |
| a commit | `CommitSeq(n)` | `--at-commit <N>` | right after commit N |
| a time | `Time(micros)` | `--at-time <TIME>` | after the last commit at or before TIME |
| a mark | `Mark(name)` | `--at-mark <NAME>` | at the mark (right after the commit it points at) |

**Every commit is a target**, not only snapshots and marks. A target is resolved the same way for
every action: the nearest snapshot at or before it is loaded and the WAL is replayed up to the
target, following the lineage chain across forks and in-place rollbacks. A target needs the
snapshot and WAL that lead to it: after a [prune](store_prune.md), targets before the oldest kept
snapshot are gone (`PersistError::TargetNotReached`).

## The time format

`TIME` is an RFC 3339 / ISO 8601 date-time **with a zone**, or microseconds since the Unix epoch:

```text
2026-10-08T02:43:00Z          UTC
2026-10-08T02:43Z             seconds may be left out
2026-10-08T04:43:00+02:00     an offset
2026-10-08T02:43:00.250Z      a fraction of a second
1760000000000000              microseconds since 1970-01-01T00:00:00Z
```

- A time **without a zone** is refused (it would depend on the machine's time zone), and so is
  a **bare date**: a day is not a moment, and whether its start or its end is meant, in which zone,
  changes the target. The error shows the explicit forms (the same text in the CLI and the dev
  server's `{"time": ..}`):

  ```text
  --at-time: "2026-10-08" is a date without a time and a zone, which is refused (it names no moment): write 2026-10-08T23:59:59Z for the end of that day in UTC, or with an offset, e.g. 2026-10-08T00:00:00+02:00 for its start at UTC+02:00 (RFC 3339)
  ```

  Python takes a time only as microseconds since the epoch (`time=`), so no text is parsed there.
- A time covers the whole second (or minute) it names: `2026-10-08T02:43Z` means up to
  02:43:59.999999. So a time copied from `store list` or `store marks`, which print whole seconds
  in this format, includes the commit printed with it.
- Times are printed in UTC (`2026-10-08T07:05:08Z`). The dev server's **Go to time...** takes a
  date and time in the browser's time zone.

## What you can do with a target

| Action | Library | CLI | Dev server (Store menu) |
|---|---|---|---|
| look at it, read-only | `Store::open_read_only(dir, target)`, `store.read_only_at(target)` | fork it, then open the fork | **Open read-only here** |
| branch it into a new store | `store.fork(target, new_dir)` | `graphersal store fork <DIR> <NEW_DIR> TARGET` | **Fork to a new directory...** |
| take THIS store back to it | `store.rollback_to(target, mode)` | `graphersal store rollback <DIR> TARGET` | **Roll back here...** |

## Read-only views

```rust,no_run
use graphersal::persist::{RecoveryTarget, Store};

let past = Store::open_read_only("data", RecoveryTarget::CommitSeq(12))?;
println!("commit {}: {} vertices", past.report.commit_seq, past.graph.vertex_count());
let mut graph = past.graph;                  // an unpublished TraversalGraph: query it, export it
let names = graph.traversal_mut().v(None::<()>).values("name").to_list()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Store::open_read_only` takes no lock and works while a writer has the store open, also on a
backup. It returns the graph with a `RecoveryReport`; the graph has no journal, so changes to it
are not stored anywhere (attach a journal only as a new history: a new lineage). A commit after
the end of the history, or a mark that does not exist, is an error
(`PersistError::TargetNotReached`); a time after the last commit is the latest state.

In the dev server, **Open read-only here** makes the server serve that state instead of the live
graph: queries, the drawing, the schema and the statistics show it, every write is refused
("the served state is a read-only view at commit N ...: writes are refused"; its help points to
**Back to the live graph** and to a fork), and a banner offers **Back to the live graph**. The
view is shared by every browser tab and the MCP agent. See
[Dev Server and MCP](dev_server.md#on-a-store).
