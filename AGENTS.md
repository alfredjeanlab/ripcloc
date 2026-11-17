# ripcloc

Counts lines of code by language, split into source and tests. There is no configuration; everything is detected.

    make check    # fmt, clippy with warnings denied, and every test

Unit tests go in a sibling `foo_tests.rs`, included with `#[cfg(test)] #[path = "foo_tests.rs"] mod tests;`. The
black-box tests in `tests/cli.rs` run the binary against `tests/fixtures/`.

  - A commit message is one line, `type(scope): ` and a sentence. The type is one of `feat`, `fix`, `perf`,
    `refactor`, `docs`, `test` or `chore`.
  - Never use `git stash`, `git reset` without `--soft`, or other lossy git operations. Commit with
    `git commit -m "…" -- <paths>`.
  - Comments and docs are ordinary English sentences. A doc comment says what the code and names cannot, such as an
    invariant, an edge case or a reason, and has no history.
