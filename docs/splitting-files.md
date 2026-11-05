# Splitting a file that outgrew the limit

`.githooks/pre-commit` blocks a commit that stages a file over 5,000 tokens,
estimated as bytes ÷ 4. The hook's `max_tokens` is the authority; this doc is the
remedy. Run `git commit` again to see the list it refused.

## Pick the branch

- **Hand-written** — source, tests, a page with behaviour in it. Split it, below.
  This is the normal case.
- **Generated, vendored, or a fixture nobody hand-writes** — add a narrow glob to
  `skip_globs` in `.githooks/pre-commit` so the hook stops asking: e.g.
  `'tests/fixtures/*.json'`, not `'*.json'`. A broad glob silently silences the
  guard for files that do need splitting.
- **A mechanical commit that cannot be split** — a pure rename, a move, import
  churn. Bypass once with `git commit --no-verify`, then split it in the next
  commit. Never let a second commit ride through on the same bypass.

## The split

1. **List the jobs.** Read the file and name each responsibility: state, input
   handling, layout, painting, persistence, helpers. One sentence per job.

2. **Pick the job with the cleanest boundary** — fewest references to the rest —
   and give it a module next to the original. For `src/app.rs` use
   `src/app/selection.rs`; for a flat module, `src/selection.rs`.

3. **Move, do not copy.** `git mv src/app.rs src/app/mod.rs` when most of the
   file belongs in one place, or create the new module and cut one block at a
   time so history follows the code.

4. **Keep the old paths working.** Re-export what callers already import
   (`pub use selection::*;` in `src/app/mod.rs`) so the split needs no changes
   elsewhere. Narrow the surface later if you want.

5. **Move one job, then check.** Run `cargo check` plus the tests for the crate
   you touched (`cargo test -p avar`, `cargo test -p rune-tty`, …). One green
   step per job beats one large rewrite.

6. **Repeat until every listed file is under the limit**, then `git add -u` plus
   the new modules. The hook measures the staged blob, so re-staging is part of
   the fix.

**Done when** every file in the hook's list is under the limit and staged,
`cargo check` passes, and the tests for each crate you touched pass.

## Splitting a test file

Same procedure, different units: group tests by the behaviour they cover, move
each group behind a shared helper module, and keep the test names — they are the
record of what is covered.
