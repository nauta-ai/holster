# Holster — Project Framework

**Owner:** Dave Nauta (CEO, user-zero)
**Builder:** Alex (DGX1, .204)
**Reviewer:** Claude Code on .203
**Repo location:** `~/holster/` on .203 (local, private)
**Vault docs:** `~/obsidian-vault/Nauta-AI/Projects/Holster/`

## What Holster is

A local-first macOS menu bar app that manages API keys for AI developers. Think 1Password, but built for the AI era — provider-aware, expiry-tracking, leak-scanning, and with real spend visibility.

**Tagline:** *Your API keys, holstered. Local-first, AI-native.*

## Why this exists

Every indie AI developer has the same set of pains:
1. Keys leaked to GitHub commits
2. Zombie keys still active months after a project died
3. No clue what they're spending across 8+ providers
4. Juggling .env files, 1Password, raw text files

Generic password managers don't solve the AI-specific pains. Enterprise tools (Doppler, Infisical, HashiCorp Vault) are overkill and don't run local-first.

## Read order

1. `01_SPEC.md` — product vision, target user, non-goals
2. `02_ARCHITECTURE.md` — stack, vault format, encryption
3. `03_THREAT_MODEL.md` — what we defend against, what we don't
4. `04_REPO_STRUCTURE.md` — directory tree
5. `05_MILESTONES.md` — 8 milestones with acceptance criteria
6. `06_MILESTONE_1_TASKS.md` — first milestone, fully scoped
7. `07_SECURITY_REVIEW_CHECKLIST.md` — CC's rubric
8. `08_BRAND.md` — name, tagline, palette, copy seed
9. `09_ALEX_KICKOFF.md` — exact commands to start

## Operating principles

- **Local-first.** Keys never leave the user's machine without explicit opt-in.
- **Zero-knowledge where possible.** If we add cloud sync, it's E2E encrypted.
- **Approval before execution.** CC reviews every milestone before merge.
- **No `unwrap()` in security-critical paths.** Errors are explicit.
- **Decrypted keys exist in memory for the shortest possible window.**
