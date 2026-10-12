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

- the CI workflow, started by hand with `full` (`gh workflow run CI -f full=true`: Linux and Windows), is green:
  format, clippy, every test suite incl. the TinkerPop suite, the feature and wasm32 matrices, the MSRV, the
  playground tests;
- the checks CI leaves out pass locally: `RUSTDOCFLAGS="-D warnings" cargo doc` with the feature sets of
  `CLAUDE.md`, `mdbook build book`, and the TinkerPop suite on the foreign storage
  (`GRAPHERSAL_TEST_STORAGE=forwarding`);
- `cargo deny check` (licences, RustSec advisories, duplicate crates, sources) passes against `deny.toml`;
- the long fuzz budgets pass: robustness, the merge checkpoint and the verified backups (below; a fast budget of
  each runs in every `cargo test`);
- a benchmark run on a quiet machine shows no unexplained regression against the previous release;
- `cargo publish --dry-run` succeeds for every published crate, `graphersal` first, then the crates that depend on it;
- the version's release notes (`book/src/release_notes/`, for 0.1.0
  [Release Notes 0.1.0](../release_notes/release_notes_0.1.0.md)) describe what changed; they are the project's
  record of changes (there is no separate `CHANGELOG.md`) and the text of the GitHub Release.

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

## Publishing

1. Tag the release commit: `git tag -a v0.1.0 -m "Graphersal 0.1.0" && git push origin v0.1.0` (the tag equals the workspace `version`).
2. The crates go to crates.io by hand, in dependency order: `cargo publish -p graphersal`, then
   `graphersal-storage-tests`, `graphersal-session` and `graphersal-cli`.
3. The Python package goes to PyPI through the workflow `release.yml`, started on the tag:
   `gh workflow run Release --ref v0.1.0`. It checks that the tag matches the version, builds the five-platform
   wheels and the sdist through `python.yml` (which also runs the binding's tests and installs each wheel it can
   run), and uploads them with PyPI Trusted Publishing from the environment `pypi` (no token is stored).
4. The GitHub Release takes its text from the release notes.
