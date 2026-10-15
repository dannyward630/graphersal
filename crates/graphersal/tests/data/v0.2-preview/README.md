# Files written by the Graphersal 0.2 line

Real output of the 0.2 development line, used by `tests/all/newer_format_tests.rs` to check
that the 0.1.x line refuses files of a NEWER format cleanly (`PersistError::UnsupportedVersion`,
"written by a newer Graphersal"), never as damage, never in maintenance mode, never repaired.

| File | Format versions |
|---|---|
| `store/` | a store directory: `GRAPH` store format version 3; manifest, segments and the WAL segment file format version 2 |
| `graph.gstore` | the same as a single-file store (container version 1, the store inside version 3 / 2) |
| `backup.zip` | a ZIP backup of `store/` (store format version 3 inside) |
| `graph.gsnap` | a packed snapshot of the modern sample graph, file format version 2 |

Made with the CLI built from branch `integration-0.2` at commit `170a36f`
(`cargo build -p graphersal-cli` in a temporary worktree), in an empty directory:

```sh
graphersal store create store --from modern
graphersal --graph store -e 'g.add_v("person").property("name", "zoe").iterate()'
graphersal store create graph.gstore --from modern
graphersal --graph graph.gstore -e 'g.add_v("person").property("name", "zoe").iterate()'
graphersal store backup store backup.zip --zip
graphersal --graph modern -e 'g.export_snapshot("graph.gsnap")'
rm store/LOCK store/BACKUP   # empty lock files, left out of the fixture
```

Never open these files read-write with a 0.2 build in place: tests compare them byte for byte
(they copy the store first and check that nothing changed).
