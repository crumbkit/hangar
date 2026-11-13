# Working on hangar

hangar serves agent-written pages from a board and works on board records
through them. It is a Cargo workspace with one crate so far, `crates/hangar-core`,
which holds the types and traits that every other hangar crate codes against.
The rules are the Rust standards that [rust-standards.toml](rust-standards.toml)
adopts. A change is ready when the pull-request lane passes. The commands of the pull-request lane are the steps of the `lane` job in
[.github/workflows/ci.yml](.github/workflows/ci.yml). Run them with
`CARGO_BUILD_WARNINGS=deny`, as that file's `env` sets.

- Write the test first, named as a sentence (BASE-36).
- Give each new dependency a reason in `Cargo.toml` (BASE-23).
- A new implementation of `Authenticator`, `PageStore` or `RecordStore` calls
  every function of the matching suite in `hangar_core::contract` from its own
  tests.

## Writing

Docs, comments and commit messages use plain declarative sentences with a
named subject. Avoid "not X but Y", fragments, metaphors and closing
flourishes. A stated limit says what it counts and what happens when it is
hit. Commit messages describe changed behaviour.
