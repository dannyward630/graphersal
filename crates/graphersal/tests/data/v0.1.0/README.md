# Golden fixtures of Graphersal 0.1.0

Stores written by the RELEASED Graphersal 0.1.0 binary (store format version 2, file format
version 1), for the store converter's tests (`tests/all/store_upgrade_tests.rs`). Every build
must keep upgrading them (book: Persistence > Store format versions). The tests copy them into a
temporary directory first: these files never change.

| Fixture | What it holds |
|---|---|
| `store/` | A directory store: the modern graph (snapshot at commit 0), a schema (mode `open`), a saved query `older`, a compression rule `bio`, the marks `first` (4), `before-rollback` (6) and `last` (7), a merged checkpoint `base` at commit 4, WAL after it, and a rollback to `before-rollback`: commits 7..=8 (vertex `engine`, `marko`'s age 30) are in the attic, a new lineage continues with commit 7 (vertex `grace`). |
| `backup/` | A full backup directory of `store/` (its `GRAPH` carries the backup marker at commit 7). |
| `backup.zip` | A ZIP backup of `store/`. |
| `fresh/` | The modern graph, opened once without a commit: its only WAL segment (`1`) holds a version 1 header and no record. |
| `single.gstore` | A single-file store: the modern graph, commit 1 (vertex `ada`, age 36), the mark `m1` (1), commit 2 (`ada`'s age 37). |

They were produced by `make.sh` with the binary built from the tag `v0.1.0`
(`git worktree add <dir> v0.1.0`, `cargo build -p graphersal-cli` there):

```sh
crates/graphersal/tests/data/v0.1.0/make.sh <dir>/target/debug/graphersal
```

The script lists every command. The lock files (`LOCK`, `BACKUP`) are removed afterwards; they
hold no data. Regenerating changes ids and times, so the tests read those from the fixtures'
own `GRAPH` files instead of pinning them; the commits, marks and values above are pinned.
