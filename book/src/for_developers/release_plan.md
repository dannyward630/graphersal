# Releases and Versioning

## Versioning

Graphersal follows [Semantic Versioning](https://semver.org). Before 1.0 a minor version (0.1 to 0.2) may break
the API; a patch version (0.1.0 to 0.1.1) does not.

Every crate of the workspace shares one version number (`[workspace.package]` in the root `Cargo.toml`), and the
Python package and the web playground are released with the same number:

| Artifact | Where | Notes |
|---|---|---|
| `graphersal` | crates.io | the library: engine, Rhai DSL, I/O, persistence (feature flags) |
| `graphersal-session` | crates.io | the playground session logic, published because the CLI depends on it; internal, no stability guarantees |
| `graphersal-cli` | crates.io | the `graphersal` binary: REPL, one-shot runner, `graphersal store`, the dev server |
| `graphersal-storage-tests` | crates.io | the `GraphStorage` conformance suite, EXPERIMENTAL like the trait (pin the exact version) |
| `graphersal` (Python) | wheels (abi3, Python 3.10+) | the PyO3 binding from `bindings/py-graphersal` |
| web playground | static files | runs `graphersal-wasm` (not published; its JS API is internal to the playground) |

## Stability in 0.1.x

- Stable within 0.1.x (additive changes only): the public API of `graphersal` outside the items marked
  EXPERIMENTAL, the DSL and the schema format.
- EXPERIMENTAL (may change in any 0.1.x release): the `GraphStorage` trait and its associated types,
  `PersistentStorage`, `StoreDir`, and `graphersal-storage-tests`.
- Internal: `graphersal-session`, the wasm crate's JS API, the dev server's HTTP API (`--server` is a development
  tool, bound to the loopback interface).
- The storage format has its own version number ([Format Versions and Upgrade](../persistence/store_versions.md)).

The API rules that keep a change additive (public enums and result structs `#[non_exhaustive]`, default methods
for every new trait method) are checked with [`cargo-semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks):

```bash
cargo install --locked cargo-semver-checks          # one-time
cargo semver-checks --package graphersal --baseline-rev <previous release tag>
```

From 0.1.0 on a reported break is a release blocker unless the version number says so.

## Release gates

Before a version is tagged:

- the CI workflow is green (format, clippy, every test suite incl. the TinkerPop suite and the foreign-storage
  proof, the feature and wasm32 matrices, the playground tests);
- `cargo deny check` (licences, RustSec advisories, duplicate crates, sources) passes against `deny.toml`;
- the long fuzz budgets pass: robustness, the merge checkpoint and the verified backups (below; a fast budget of
  each runs in every `cargo test`);
- a benchmark run on a quiet machine shows no unexplained regression against the previous release;
- `cargo publish --dry-run` succeeds for every published crate, `graphersal` first, then the crates that depend on it;
- `CHANGELOG.md` has the version's section with its date, and the [release notes](../release_notes/release_notes_0.1.0.md)
  summarize it.

The long fuzz budgets (release builds with overflow checks and debug assertions kept on):

```bash
CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true \
ROBUSTNESS_CASES=500000 ROBUSTNESS_SEED=23 ROBUSTNESS_THREADS=7 \
  cargo test --release -p graphersal --features script,io,display,serde,persist \
  --test all robustness_fuzz_tests::fuzz_long -- --ignored
ROBUSTNESS_INPUT_CASES=20000 cargo test --release -p graphersal \
  --features script,io,display,serde,persist --test all robustness_input_tests::
MERGE_FUZZ_CASES=60000 MERGE_FUZZ_SEED=1 MERGE_FUZZ_THREADS=8 cargo test --release -p graphersal \
  --features script,io,display,serde,persist,persist-zip --test all merge_checkpoint_fuzz_tests::
BACKUP_FUZZ_CASES=60000 BACKUP_FUZZ_SEED=1 BACKUP_FUZZ_THREADS=8 cargo test --release -p graphersal \
  --features script,io,display,serde,persist,persist-zip --test all backup_fuzz_tests::
```

Robustness findings go to `target/robustness-findings.txt` (`ROBUSTNESS_REPLAY=<seed>` replays one case); a failing
merge or backup case prints `MERGE_FUZZ_REPLAY=<seed>` or `BACKUP_FUZZ_REPLAY=<seed>`.
