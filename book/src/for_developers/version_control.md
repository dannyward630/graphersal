# Version Control and Contributions

The short human guide for contributors is `CONTRIBUTING.md` in the repository root; this page summarizes how
changes reach the repository.

## Branches and pull requests

- `main` is the only long-lived branch; every change comes as a pull request against it.
- A pull request keeps the CI workflow green (format, clippy, the test suites, the feature and wasm32 matrices);
  CI runs on a manual start, so run the checks locally (`CONTRIBUTING.md`) and a maintainer starts CI for the
  pull request. When it adds or fixes a Gremlin step, the TinkerPop baseline is updated in the same pull request
  ([TinkerPop Compliance](tinkerpop_compliance.md)).
- A user-visible change is described in the upcoming version's release notes (`book/src/release_notes/`).
- Releases are tags on `main` ([Releases and Versioning](release_plan.md)).

## Commit messages

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/), in English:
`type(scope): summary`, for example `fix(step): has_id() with a numeric predicate` or
`docs(book): the Store's backup page`. Common types are `feat`, `fix`, `docs`, `test`, `refactor`, `perf`,
`build` and `chore`.

## Sign-off (DCO)

Graphersal is licensed under Apache-2.0, and contributions are accepted under the same licence; there is no
Contributor License Agreement. Instead, every commit carries a sign-off that certifies the Developer Certificate of
Origin 1.1 (the `DCO` file in the repository root):

```bash
git commit -s -m "fix(step): ..."
# adds:  Signed-off-by: Your Name <you@example.com>
```

The name and e-mail must match the commit author. A CI check rejects pull requests with commits that are not signed
off; to fix an existing branch, run `git rebase --signoff <base>` and force-push it.
