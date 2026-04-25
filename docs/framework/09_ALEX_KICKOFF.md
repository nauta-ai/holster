# 09 — Alex Kickoff

This is the first thing Alex executes. Each block runs as-is on .203.

---

## Pre-flight checks

```bash
# Verify Alex is on .203
hostname

# Verify required tools
which cargo rustc node pnpm git
rustc --version          # need 1.78+
node --version           # need 20+
pnpm --version           # need 9+

# If any missing:
# rustup install stable
# brew install pnpm node
```

---

## Step 1: Create the repo

```bash
cd ~
mkdir -p holster
cd holster
git init -b main
git config user.name "Alex (Nauta Builder)"
git config user.email "alex@nautaai.com"
```

---

## Step 2: Drop the framework docs in place

```bash
# Copy the framework docs from the vault
mkdir -p docs/framework
cp -r ~/obsidian-vault/Nauta-AI/Projects/Holster/* docs/framework/

ls docs/framework/
# Should list: 00_README.md through 09_ALEX_KICKOFF.md
```

---

## Step 3: Initial commit + branches

Use Python (not nano/heredoc) for any file edits per ops policy.

```bash
python3 - <<'PY'
from pathlib import Path

Path(".gitignore").write_text("""target/
dist/
node_modules/
*.db
*.db-journal
*.db-shm
*.db-wal
.env
.env.local
.DS_Store
*.log
logs/
.vscode/
.idea/
""")

Path("README.md").write_text("""# Holster

Local-first API key manager for AI developers.

Read `docs/framework/00_README.md` to start.

Status: pre-alpha, M1 in progress.
""")

Path("LICENSE").write_text("""Source-Available License v0.1

Copyright (c) 2026 Dave Nauta / Nauta AI

Permission to view source granted. Commercial use,
redistribution, and derivative works prohibited
without written permission.

Contact: dave@nautaai.com
""")

print("seed files written")
PY

git add .
git commit -m "chore: initial scaffold + framework docs"
git checkout -b dev
git checkout -b milestone/M1-vault-foundation
```

---

## Step 4: Cargo workspace + crate skeletons

Follow `06_MILESTONE_1_TASKS.md` task by task. Each task ends with an explicit acceptance check.

After completing T1.0-T1.8, run:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo audit
```

All four must succeed before requesting CC review.

---

## Step 5: Request CC review

Push the milestone branch:

```bash
git push -u origin milestone/M1-vault-foundation
# (Or for local-only: just commit; CC reads from filesystem)
```

Send Telegram ping to Dave (chat ID 8553928686):

```
M1 build complete. Ready for CC review.

Branch: milestone/M1-vault-foundation
Commit: <sha>
Tests: <N> passing, 0 failing
Clippy: clean
Audit: clean

CC review request: please run security checklist M1 sections.
```

---

## Step 6: Address CC findings

CC produces `docs/reviews/M1-review-YYYYMMDD.md`. Alex:

1. Reads the report
2. For each blocking finding: fix, commit with `sec: <description>`
3. For each non-blocking note: log to `docs/reviews/backlog.md`
4. Re-request CC review if any blocking finding required code changes

---

## Step 7: Dave's approval gate

Dave reviews CC's report via Mission Control or directly. If approved:

```bash
git checkout dev
git merge --no-ff milestone/M1-vault-foundation
git push
```

Then start M2 on a new branch.

---

## Operating notes for Alex

- **No nano/heredoc for file edits.** Use Python file writers per ops policy.
- **No inline `#` comments inside heredoc shell blocks** — use `# comment on its own line` outside the heredoc.
- **Commit cadence:** small commits, conventional format. CC reviews diffs more easily than big drops.
- **Security commits get `sec:` prefix.** This flags them for extra CC attention.
- **Test fixtures use clearly-fake key values:** `sk-ant-test-1111...`. Never use a real provider key in any test, ever.
- **If you hit an unknown:** stop, write the question to `docs/questions.md`, ping Dave on Telegram. Do not guess on security-relevant code.

---

## Definition of done for the entire framework intake

- [ ] Repo scaffolded at `~/holster/` on .203
- [ ] Framework docs copied to `docs/framework/`
- [ ] Cargo workspace compiles
- [ ] M1 branch created
- [ ] Alex has read all 9 framework docs at least once
- [ ] Alex has flagged any unclear sections in `docs/questions.md` before starting code

Once that's done, M1 work begins.
