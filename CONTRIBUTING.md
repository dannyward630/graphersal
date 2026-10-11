# Contributing to Graphersal

Thanks for your interest. Graphersal is an in-memory graph traversal engine in Rust, modelled on Apache TinkerPop
Gremlin. The agent-oriented rules live in `CLAUDE.md`; this file is the short human version.

## Build and test

Rust 1.88 or newer. The workspace also contains the PyO3 binding, which needs a Python 3.10+ interpreter at build time
(set `PYO3_PYTHON` if your default `python3` is older, or add `--exclude py-graphersal`).

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test  --package graphersal --features script,io,display,serde,persist   # includes the TinkerPop suite
cargo test  --package graphersal --doc --features script,io
cargo test  --package graphersal-cli
cargo test  --package graphersal-storage-tests
cargo deny check                                                          # cargo install --locked cargo-deny
```

Feature hygiene (after touching `cfg(feature)` code or dependencies): `cargo check --package graphersal` for each of
`--no-default-features`, `--features script`, `--features display`, `--features serde`, `--features io`,
`--features persist`, `--features persist-zstd`, `--features persist-zip`, `--all-features`, and
`--no-default-features --features script,serde,display`. The library must also build for
`wasm32-unknown-unknown` (`rustup target add wasm32-unknown-unknown`, then the same `cargo check` with `--target`).

Try a query: `cargo build -p graphersal-cli && target/debug/graphersal -e 'g.v().has_label("person").values("name").to_list()'`.

## Robustness (fuzz) tests

Arbitrary queries and inputs must never crash the engine: every failure is an error, never a panic, a hang or
unbounded memory. A seeded fuzzer checks this:

- The fast budget (`crates/graphersal/tests/all/robustness_*`) runs with every `cargo test` above; a pull request must
  keep it green.
- If your change touches steps, the Rhai bindings, value conversion, parsing (jpath, GraphML, schemas, change sets) or
  limits, also run the long budget locally before opening the pull request (about 10-15 minutes on a laptop):

```bash
CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true \
ROBUSTNESS_CASES=500000 ROBUSTNESS_SEED=23 ROBUSTNESS_THREADS=4 \
  cargo test --release -p graphersal --features script,io,display,serde,persist \
  --test all robustness_fuzz_tests::fuzz_long -- --ignored
ROBUSTNESS_INPUT_CASES=20000 cargo test --release -p graphersal \
  --features script,io,display,serde,persist --test all robustness_input_tests::
```

  Findings are written to `target/robustness-findings.txt`; `ROBUSTNESS_REPLAY=<seed>` replays one case. A new
  finding caused by your change must be fixed in the same pull request. A pre-existing one goes into
  `crates/graphersal/tests/all/robustness_bug_tests.rs` as an `#[ignore = "BUG: ..."]` test with the minimized
  repro, mentioned in the pull request.
- A fix for a known robustness bug un-ignores its test there.

## The TinkerPop suite

`cargo test --package graphersal --features script,io,display,serde,persist --test tinkerpop_features` runs the vendored
Apache TinkerPop 3.8.2 scenarios through the Rhai DSL against a ratchet baseline
(`crates/graphersal/tests/tinkerpop/baseline.toml`) and writes `target/tinkerpop-report.md`. When a step is added or
fixed, remove the scenarios that now pass from the baseline (or regenerate it with `TINKERPOP_UPDATE_BASELINE=1` and
review the diff) in the same change. See `crates/graphersal/tests/tinkerpop/README.md`.

## Code rules (summary)

- No `unwrap`/`expect`/`panic!` in step or script paths; map failures to a `TraverserError`.
- A change to a Rust step also updates its Rhai binding, in both spellings.
- Steps are lazy: no per-item allocation, materialize only at terminals.
- Public API stays small; public enums and result structs are `#[non_exhaustive]` unless closed by definition.
- Every public item has a `///` comment; new public features get integration tests in `crates/graphersal/tests/all/`
  (one test binary; add a module, never a new top-level test file).
- Library code logs through `tracing`, never `println!`.
- Do not infer types from string content.
- Documentation, comments and commit messages are in English.

## Licence of contributions and sign-off (DCO)

Graphersal is licensed under Apache-2.0 and will stay open source under that licence. Contributions are accepted
under the same licence (Apache-2.0, section 5: "inbound = outbound"); you keep your copyright. There is no
Contributor License Agreement for this repository.

Instead, every commit carries a sign-off that certifies the [Developer Certificate of Origin](DCO) 1.1: that you
wrote the change or otherwise have the right to submit it under the project's licence. Sign off with `-s`:

```bash
git commit -s -m "fix(step): ..."
# adds:  Signed-off-by: Your Name <you@example.com>
```

The name and e-mail must match the commit author. A CI check rejects pull requests with commits that are not
signed off; to fix an existing branch, run `git rebase --signoff <base>` and force-push it.

Contributions to a future Graphersal server, which will live in its own repository, may follow a different
process; that repository will say so.
