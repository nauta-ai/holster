# 01 — Holster Product Spec

## Vision

The default place where AI developers keep their API keys. A menu bar app you forget is running because it just works — until the moment a key is about to expire or you're about to commit one to GitHub, at which point it saves you.

## Target user (v1)

- AI indie developers and small-team builders
- Mac primary
- Juggling 5-15 keys across providers like Anthropic, OpenAI, Google, Replicate, ElevenLabs, Pinecone, Stripe, Cloudflare
- Comfortable with the terminal, mostly working in VS Code / Cursor / Claude Code
- Currently using some combination of: .env files, 1Password, raw text files, paper

## Core jobs to be done

1. **Stash** — add a key, tag it, set expiry, never lose it
2. **Use** — copy or inject into shell with one click/command, auto-clear
3. **Audit** — see all keys, their status, when last used, when expiring
4. **Detect** — scan local repos for accidentally committed keys
5. **Spend** — know what each provider and project is costing this month

## Non-goals (v1)

- Not a team secrets manager (no shared vaults, no role-based access)
- Not a cloud secrets store (no server-side storage of keys)
- Not a code scanner for production CI/CD pipelines
- Not a runtime gateway/proxy for API calls
- Not Windows or Linux (v1)
- Not a credential manager for non-AI services (no SSH keys, no DB passwords)
- Not a generic password manager (passwords for websites)

## Success criteria

**Personal (Dave as user-zero):**
- Replaces my current key management within 30 days of v1
- Catches at least one zombie or expired key in my own setup
- Surfaces a spend insight I didn't already know

**Public launch (3 months post-v1):**
- 1,000 free downloads
- 100 paid Pro conversions
- Featured on at least one of: HN front page, Show HN, ProductHunt top 5 of day
- Zero security incidents (no reported key leaks attributable to Holster)

## Pricing (locked)

- **Free tier:** vault, copy/paste, expiry dates, leak scanner, basic search
- **Pro tier:** $5/mo or $49/year — spend tracking, encrypted iCloud sync, CLI, advanced filters, priority support

## Platform (locked)

- v1: macOS 13+ (Apple Silicon + Intel)
- v2 (post-launch): Windows via Tauri's existing cross-platform support
- Mobile: deferred until post-launch user demand confirms

## Out of scope until v1.5+

- Team sharing / shared vaults
- SSO / SAML
- Audit log export
- Browser extension
- Webhook integrations
