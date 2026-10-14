#!/bin/sh
# Regenerates the golden store fixtures of Graphersal 0.1.0 (store format version 2, file format
# version 1) with a RELEASED 0.1.0 binary, for the store upgrade tests
# (crates/graphersal/tests/all/store_upgrade_tests.rs). The tests compare against what this build
# reads from these bytes and pin the commits and marks made below, so regenerate them only on
# purpose and update the expectations with them.
#
#   usage: make.sh <path to the graphersal 0.1.0 binary> [<output dir, default: this dir>]
#
# Build the binary from the release tag in a separate checkout, for example:
#   git worktree add /tmp/v010 v0.1.0 && (cd /tmp/v010 && cargo build -p graphersal-cli)
#   -> /tmp/v010/target/debug/graphersal
set -eu
BIN=${1:?usage: make.sh <graphersal 0.1.0 binary> [<out dir>]}
OUT=${2:-$(cd "$(dirname "$0")" && pwd)}
"$BIN" --version | grep -q '^graphersal 0\.1\.0$' || { echo "not a 0.1.0 binary: $BIN" >&2; exit 2; }
cd "$OUT"
rm -rf store single.gstore backup backup.zip fresh
run() { "$BIN" --graph "$1" -e "$2" > /dev/null; }

# A directory store: the modern graph (its snapshot at commit 0), a schema, a saved query, a
# compression rule, marks, a checkpoint with WAL history after it, and a rollback whose
# rolled-back commits stay in the attic; a new lineage continues after it. Small WAL segments
# would need an option the CLI does not have: the history is one segment per lineage.
"$BIN" store create store --from modern > /dev/null
run store 'g.set_schema(g.infer_schema(), SchemaMode.open)'
run store 'g.define_query(#{name: "older", params: #{min: #{schema: #{type: "integer"}, default: 30}}, body: "fn older(min) { g.v().has(\"age\", P.gt(min)) }"})'
run store 'g.define_compression(#{name: "bio", label: "person", path: "bio", min_bytes: 16})'
run store 'g.add_v("person").property("name", "ada").property("age", 36).property("bio", "Ada Lovelace wrote the first program for the Analytical Engine.").iterate()'
"$BIN" store mark store first > /dev/null
"$BIN" store checkpoint store --name base > /dev/null
run store 'g.add_v("person").property("name", "alan").property("age", 41).iterate()'
run store 'g.v().has("name", "alan").add_e("knows").to(__.v().has("name", "ada")).property("weight", 0.5).iterate()'
"$BIN" store mark store before-rollback > /dev/null
run store 'g.add_v("software").property("name", "engine").property("lang", "babbage").iterate()'
run store 'g.v().has("name", "marko").property("age", 30).iterate()'
"$BIN" store rollback store --at-mark before-rollback > /dev/null
run store 'g.add_v("person").property("name", "grace").property("age", 85).iterate()'
"$BIN" store mark store last > /dev/null

# A backup directory and a ZIP backup (0.1.0 builds the CLI with persist-zip).
"$BIN" store backup store backup > /dev/null
"$BIN" store backup store backup.zip --zip > /dev/null

# A single-file store with WAL after its snapshot and a mark.
"$BIN" store create single.gstore --from modern > /dev/null
run single.gstore 'g.add_v("person").property("name", "ada").property("age", 36).iterate()'
"$BIN" store mark single.gstore m1 > /dev/null
run single.gstore 'g.v().has("name", "ada").property("age", 37).iterate()'

# A store opened once without a commit: its only WAL segment holds no record.
"$BIN" store create fresh --from modern > /dev/null
run fresh 'g.v().count().next()'

# The lock files carry no data (an open recreates them).
rm -f store/LOCK store/BACKUP backup/LOCK backup/BACKUP fresh/LOCK fresh/BACKUP
echo "fixtures written to $OUT"
