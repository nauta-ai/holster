# 08 — Brand

## Name

**Holster**

## Domain

Primary: `holster.dev`
Backup: `getholster.app`, `holsterkeys.com`

(Alex: register `holster.dev` early, before launch.)

## Tagline

**"Your API keys, holstered."**

Sub-line: *Local-first key management built for the AI era.*

## Positioning

Holster is to AI keys what 1Password was to website passwords ten years ago — the obvious right answer once you've used it. Built specifically for indie AI developers who are tired of leaking keys, losing track of spending, and juggling .env files.

## Voice

- Confident, dry, technical
- Talks to devs as peers
- Never patronizing, never marketing-flowery
- Avoid: "revolutionary," "game-changing," "unleash," "supercharge"
- Use: "your," "stop," "know," "see"

## Color palette

| Role | Color | Hex |
|---|---|---|
| Primary (steel) | Cool gunmetal gray | `#2A2F36` |
| Accent (signal) | Tactical orange | `#E8743B` |
| Background light | Off-white | `#F7F6F3` |
| Background dark | Near-black | `#14161A` |
| Status: active | Green | `#4ADE80` |
| Status: warning | Amber | `#F59E0B` |
| Status: expired | Red | `#EF4444` |
| Status: stale | Gray | `#6B7280` |
| Text primary | `#1F2937` (light) / `#F3F4F6` (dark) | |
| Text secondary | `#6B7280` (light) / `#9CA3AF` (dark) | |

The metaphor leans tactical/utility — leather + steel, not consumer-glossy. Think: tool you keep on your hip, not toy on your desktop.

## Typography

- **Display / headers:** Inter (700, 600)
- **Body:** Inter (400, 500)
- **Mono (key values, code):** JetBrains Mono

## Iconography

- Menu bar icon: simple holster silhouette, monochrome (template image so it adapts to light/dark)
- App icon: holster + key motif, modern flat, no skeuomorphic leather textures
- Provider logos: official, full color, but rendered at 16-20px so they read as accents not focal points

## Landing page copy seed

### Hero

# Your API keys, holstered.

Stop leaking keys to GitHub. Stop guessing what you're spending.
Stop losing track of which key powers which project.

Holster is a local-first menu bar app that keeps your AI API keys
encrypted on your Mac — and tells you exactly what they're costing
you, when they expire, and which ones you forgot to rotate.

[ Download for macOS ]   [ View on GitHub ]

### Three-up

**Local-first.**
Your keys never touch our servers. We literally cannot read them.

**Provider-aware.**
Holster knows the difference between an Anthropic key and an
OpenAI key — and tracks usage and spend for both.

**Hygiene built in.**
Expiry dates. Last-used tracking. Repo leak scanning.
Nothing gets forgotten in a `.env` file again.

### Why I built this

Hi, I'm Dave. I'm a Dell Technologies strategic account manager
who builds AI side projects in his spare time. I've leaked keys
to GitHub more times than I want to admit, paid for zombie keys
I forgot existed, and lost an embarrassing amount of money to
"why is OpenAI charging me $400 this month."

Holster is the tool I wish I had two years ago.

It's local. It's mine. And now it's yours.

### Pricing

**Free**
- Encrypted local vault
- Unlimited keys
- Expiry tracking
- Repo leak scanner
- Forever, no email required

**Pro — $5/month or $49/year**
- Spend tracking for Anthropic, OpenAI, and 6 more
- CLI companion (`holster use anthropic`)
- End-to-end encrypted iCloud sync
- Advanced search and filters
- Priority support

[ Start free ]

### Footer trust signals

- Open architecture: read our [security model]
- Audited dependencies, zero analytics, zero telemetry by default
- Built by [@dnauta](https://x.com/...) — Dell SAM, indie AI builder, Killeen TX

## Launch posts (templates)

### HN Show post

```
Show HN: Holster – local-first API key manager for AI developers

I kept leaking keys to GitHub and getting surprise OpenAI bills,
so I built the tool I wanted: a Mac menu bar app that encrypts
your AI API keys locally, tracks expiry and spend, and scans
your repos for accidentally committed secrets.

Built with Tauri (Rust + React). Vault is SQLCipher + per-key
AES-256-GCM. Argon2id KDF. Zero analytics. No cloud sync unless
you turn it on, and even then it's E2E encrypted.

Free tier covers 95% of solo dev needs. Pro adds spend tracking
and a CLI companion.

Looking for security review and feedback. Source-available repo
linked below.

https://holster.dev
```

### X/Twitter launch thread

```
1/ I leaked an Anthropic key to a public repo last month.

It cost me $0 — I caught it in 4 minutes — but the whole class
of "where are my AI keys, when do they expire, what do they
cost me" problem has been bugging me for a year.

So I built Holster.

2/ Holster is a Mac menu bar app for managing API keys.
Local-first. Encrypted. Provider-aware.

It does three things 1Password doesn't:
- Tracks expiry per key
- Scans your local repos for leaked keys
- Shows you what each key is costing you this month

3/ Stack:
- Tauri 2 (Rust backend, React frontend)
- SQLCipher whole-DB encryption
- AES-256-GCM per-key encryption
- Argon2id KDF, OWASP 2024 params
- Zero analytics, zero telemetry by default

4/ Free tier is genuinely useful — vault, expiry, leak scanner.

Pro ($5/mo) adds spend tracking and a CLI you'll actually use:
`eval $(holster use anthropic)` and the key is in your env,
never on disk.

5/ Built it because I wanted it. Eating my own dog food daily.

If you're an indie AI dev tired of .env files and surprise bills,
this is for you.

holster.dev
```
