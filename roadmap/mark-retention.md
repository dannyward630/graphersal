# Kept marks: prune that preserves named points (backlog)

Status: BACKLOG for a release after 0.1.0. Nothing here is decided
beyond what "Already in place" describes. Scheduling a prune (cron-like jobs, automatic retention)
is NOT a library feature: it belongs to the server, in its own repository; the library only
provides the operations and the reports a scheduler calls.

## Why

A mark names a position in the history ("before-import", "release-1"). It needs a snapshot at or
before it and the WAL from there to it. `prune(up_to)` only knows commits: whatever it removes, a
mark before the oldest kept snapshot loses its history. A user who sets a mark expects to find it
again; with a regular prune (a server's retention job) the marks would silently go.

## Already in place (0.1.0)

- Listings (`Store::marks`, `StoreInfo::marks`, `store marks|info`, the Store menu, Python) leave
  out marks before the oldest snapshot; opening one fails with "a prune removed the history it
  needs"; its name stays taken (format spec 10).
- `PruneReport::marks_unreachable` names the marks a prune made unreachable; the compaction advice
  (`advice.prune.marks_unreachable`, the Store menu's Disk space block) names them before.
- The `marks` file is not rewritten by a prune: a backup with older snapshots still reaches them.

## Idea

`g.mark("release-1", #{keep: true})` / `store.mark_with(name, MarkOptions::keep())`: prune keeps
what a kept mark needs, everything else between kept marks may go ("compaction between marks").

- **What to keep for a kept mark M**: a snapshot S <= M and the whole WAL from S to M (replay cannot
  skip commits); the WAL after M up to the next kept point may go. Better: a snapshot exactly at M
  (written when the mark is kept, or lazily by the first prune that needs it); then M needs no WAL
  at all and every segment between kept snapshots may go. The WAL is removed per segment (64 MiB),
  so without a snapshot at M a segment partly in the needed range stays whole.
- **Cost**: each kept mark holds about one snapshot (less with incremental snapshots that share
  untouched segments through holders, format spec 6.2). The compaction advice should report the
  bytes the kept marks hold.
- **Where the flag lives**: in the Mark WAL record (an optional payload field, format spec 16: no
  new version), because the `marks` file is derived and rebuilt from the WAL. The record survives
  prune because the WAL up to M is kept (or the segment holding the record, with a snapshot at M).
  Releasing a kept mark needs its own record (`unkeep`), otherwise it is held forever.
- **Name**: `keep`, not "durable" (`Durability`, `DurableEnd`, `DurablePosition` mean fsync).
- **Interplay**: `plan_prune` gets the kept marks as one more source of "keep from" next to the
  attic bases (`kept_for_attic` -> `kept_for_marks`); `prune_backup` follows the same rule in the
  backup; a rollback before a kept mark moves it into the attic with its history (unchanged);
  `verify` checks that every kept mark is reachable; `store info` and the Store menu show which
  marks are kept and the bytes they hold.
