# Operating notes — anti-hallucination rules

Read this before doing any work in this repo.

## Verification beats narration

Every claim must come with a verifiable artifact:
- "I committed X" → show the commit hash (`git log -1`)
- "Tests pass" → show the actual `cargo test` output, all of it
- "Cargo audit is clean" → show the `cargo audit` output, all of it
- "Clippy is clean" → show the `cargo clippy --workspace --all-targets -- -D warnings` output

If you can't show me the artifact, the work didn't happen.

## Scope discipline

Do **one task at a time** from `docs/framework/06_MILESTONE_1_TASKS.md`.
Each task ends with an explicit acceptance check. Do not start the next
task until the current one's acceptance check passes verifiably.

## When you don't know

Stop. Write the question into `docs/questions.md`. Ping Dave on Telegram
(chat 8553928686). **Do not invent.** Especially in `crates/holster-vault/`,
which is security-critical.

## Forbidden patterns

- `unwrap()` in security-critical paths (vault, crypto, key handling).
  Use `?` with explicit error types.
- Storing decrypted keys in long-lived variables. Decrypt → use → drop.
- Real provider keys in tests. Use `sk-ant-test-1111...` style fakes.
- `nano` / inline heredocs for code edits. Use Python writers.
- `# inline comments` inside heredoc shell blocks (per ops policy).

## Commit hygiene

- Conventional commits (`chore:`, `feat:`, `fix:`, `sec:`, `test:`, `docs:`).
- Security commits MUST use `sec:` prefix — flags for extra CC review.
- Small commits beat big drops. CC reviews diffs more easily.

## Definition of "milestone done"

A milestone is done when:
1. All tasks have green acceptance checks
2. `cargo fmt --all` clean
3. `cargo clippy --workspace --all-targets -- -D warnings` clean
4. `cargo test --workspace` green (all tests pass, none ignored without comment)
5. `cargo audit` clean
6. CC review report exists at `docs/reviews/M{N}-review-YYYYMMDD.md` with no
   blocking findings (or all blocking findings addressed in follow-up commits)
7. Dave approves the merge to `dev`
