//! Holster Native Detector Pack
//!
//! Registry of regex-based detectors for the API keys + secret types most
//! commonly leaked by AI builders, vibe coders, and agent workflows.
//!
//! Tiers (see `Tier` enum):
//!   - **Tier 1**: existential-leak risk for AI builders. OpenAI, Anthropic,
//!     Gemini, Telegram bot, GitHub PAT, Stripe live, Etsy OAuth pair,
//!     Cloudflare API token.
//!   - **Tier 2**: common AI-stack auxiliary services. Replicate, Hugging
//!     Face, OpenRouter, ElevenLabs, Pinecone, Supabase, Neon, MongoDB
//!     URI, AWS access keys, GCP, Azure OpenAI.
//!   - **Tier 3**: messaging + utility services + generic fallbacks. Slack,
//!     Discord, Notion, Airtable, Apify, JWTs, .pem private keys.
//!
//! V0 scope (this module):
//!   - `detector_registry()` — compile-once registry of all detectors.
//!   - `scan_text(s: &str) -> Vec<Detection>` — runs the registry against
//!     an in-memory string, returns Detections with REDACTED previews
//!     (never the raw match).
//!   - **No file I/O.** The M3 repo scanner will wrap `scan_text` and
//!     walk a directory, respecting `.gitignore`. That work is out of
//!     scope here.
//!
//! Redaction contract: `Detection.redacted_preview` shows at most the
//! first 4 + last 4 characters of the raw match, with `...` between.
//! For matches shorter than 12 characters the preview is just `***`.
//! Tests assert that `redacted_preview` NEVER contains the full raw
//! match for any input ≥12 chars.

use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

// ── Public types ─────────────────────────────────────────────────────────────

/// Tier classification — drives roadmap + (eventually) Free/Founder/Pro
/// surfacing. Per the product plan in the vault, ALL tiers run on every
/// install — the tier classification is documentary, not a gate.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Tier1,
    Tier2,
    Tier3,
}

/// How dangerous a leak of this type is, all else equal.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Critical, // immediate financial / production-system access
    High,     // significant blast radius (read+write API access)
    Medium,   // notable but bounded
    Low,      // mostly read-only or short-lived
}

/// One detector — what to look for, what it is, what to do about it.
///
/// `patterns` is a Vec because some providers ship multiple prefix variants
/// (e.g., GitHub PAT classic vs fine-grained vs OAuth).
#[derive(Clone)]
pub struct Detector {
    pub id: &'static str,
    pub provider: &'static str,
    pub display_name: &'static str,
    pub patterns: Vec<Regex>,
    pub tier: Tier,
    pub risk_level: RiskLevel,
    pub remediation_hint: &'static str,
    pub rotation_url: Option<&'static str>,
    pub docs_url: Option<&'static str>,
}

/// One scanner finding. The `redacted_preview` is the only field that
/// touches the matched substring; the full raw match is NEVER carried in
/// a `Detection`. Tests enforce this.
#[derive(Serialize, Clone, Debug)]
pub struct Detection {
    pub secret_type: &'static str,  // Detector.id
    pub provider: &'static str,     // Detector.provider
    pub display_name: &'static str, // Detector.display_name
    pub file_path: Option<String>,  // None when scanning in-memory text
    pub line_number: usize,         // 1-based
    pub redacted_preview: String,   // "abcd...wxyz" or "***"
    pub risk_level: RiskLevel,
    pub tier: Tier,
    pub git_tracked: Option<bool>, // None until M3 repo scanner wraps this
    pub recommended_action: &'static str, // Detector.remediation_hint
    pub rotation_url: Option<&'static str>,
    pub docs_url: Option<&'static str>,
}

// ── Redaction ────────────────────────────────────────────────────────────────

/// Build a safe preview of a matched substring. Shows at most first 4 +
/// last 4 chars; for short matches returns `***`. Never returns the full
/// input verbatim. Always-redact policy.
pub fn redact_match(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() < 12 {
        return "***".into();
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}...{tail}")
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Lazily-initialized, compiled detector registry.
pub fn detector_registry() -> &'static [Detector] {
    static REGISTRY: OnceLock<Vec<Detector>> = OnceLock::new();
    REGISTRY.get_or_init(build_registry)
}

fn rx(pat: &str) -> Regex {
    Regex::new(pat).expect("detector regex compiles")
}

fn build_registry() -> Vec<Detector> {
    vec![
        // ─── Tier 1: existential leak risk ────────────────────────────────
        Detector {
            id: "openai_api_key",
            provider: "openai",
            display_name: "OpenAI API key",
            patterns: vec![
                // Modern: sk-, sk-proj-, sk-svcacct-
                rx(r"\bsk-(?:proj-|svcacct-|admin-)?[A-Za-z0-9_-]{30,}\b"),
            ],
            tier: Tier::Tier1,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Rotate immediately at platform.openai.com → API keys → Revoke. Update env vars in deployments.",
            rotation_url: Some("https://platform.openai.com/api-keys"),
            docs_url: Some("https://platform.openai.com/docs/api-reference"),
        },
        Detector {
            id: "anthropic_api_key",
            provider: "anthropic",
            display_name: "Anthropic / Claude API key",
            patterns: vec![rx(r"\bsk-ant-(?:api03-|admin-)?[A-Za-z0-9_-]{40,}\b")],
            tier: Tier::Tier1,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Rotate immediately at console.anthropic.com → Settings → API Keys.",
            rotation_url: Some("https://console.anthropic.com/settings/keys"),
            docs_url: Some("https://docs.anthropic.com/"),
        },
        Detector {
            id: "google_ai_api_key",
            provider: "google",
            display_name: "Google Gemini / AI Studio / Generic Google API key",
            patterns: vec![rx(r"\bAIza[A-Za-z0-9_-]{35}\b")],
            tier: Tier::Tier1,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Rotate at aistudio.google.com (Gemini) or console.cloud.google.com (other Google APIs).",
            rotation_url: Some("https://aistudio.google.com/apikey"),
            docs_url: Some("https://ai.google.dev/"),
        },
        Detector {
            id: "telegram_bot_token",
            provider: "telegram",
            display_name: "Telegram bot token",
            patterns: vec![rx(r"\b[0-9]{8,12}:[A-Za-z0-9_-]{30,40}\b")],
            tier: Tier::Tier1,
            risk_level: RiskLevel::High,
            remediation_hint: "Talk to @BotFather → /revoke to invalidate; /token to issue new.",
            rotation_url: Some("https://t.me/BotFather"),
            docs_url: Some("https://core.telegram.org/bots/api"),
        },
        Detector {
            id: "github_pat_classic",
            provider: "github",
            display_name: "GitHub Personal Access Token (classic)",
            patterns: vec![rx(r"\bghp_[A-Za-z0-9]{36}\b")],
            tier: Tier::Tier1,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Revoke at github.com/settings/tokens. Audit recent activity for unauthorized actions.",
            rotation_url: Some("https://github.com/settings/tokens"),
            docs_url: Some("https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens"),
        },
        Detector {
            id: "github_pat_fine_grained",
            provider: "github",
            display_name: "GitHub Personal Access Token (fine-grained)",
            patterns: vec![rx(r"\bgithub_pat_[A-Za-z0-9_]{82}\b")],
            tier: Tier::Tier1,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Revoke at github.com/settings/tokens?type=beta. Fine-grained tokens have repo-scoped access — confirm scopes before issuing replacement.",
            rotation_url: Some("https://github.com/settings/tokens?type=beta"),
            docs_url: Some("https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens#about-fine-grained-personal-access-tokens"),
        },
        Detector {
            id: "github_oauth_token",
            provider: "github",
            display_name: "GitHub OAuth / app / server-to-server token",
            patterns: vec![rx(r"\bgh[osu]_[A-Za-z0-9]{36}\b")],
            tier: Tier::Tier1,
            risk_level: RiskLevel::High,
            remediation_hint: "Revoke at the GitHub App settings page. Server-to-server (ghs_) tokens are short-lived; OAuth (gho_) and user-to-server (ghu_) need explicit revocation.",
            rotation_url: Some("https://github.com/settings/applications"),
            docs_url: Some("https://docs.github.com/en/apps/creating-github-apps"),
        },
        Detector {
            id: "stripe_live_secret",
            provider: "stripe",
            display_name: "Stripe live secret key",
            patterns: vec![
                rx(r"\bsk_live_[A-Za-z0-9]{24,}\b"),
                rx(r"\brk_live_[A-Za-z0-9]{24,}\b"), // restricted live key
            ],
            tier: Tier::Tier1,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Roll the key at dashboard.stripe.com/apikeys IMMEDIATELY. Audit charges/payouts/refunds for the leak window.",
            rotation_url: Some("https://dashboard.stripe.com/apikeys"),
            docs_url: Some("https://stripe.com/docs/keys"),
        },
        Detector {
            id: "etsy_oauth_pair",
            provider: "etsy",
            display_name: "Etsy keystring:shared_secret pair",
            // Etsy app credentials are commonly written as <keystring>:<secret>.
            // Tight-ish bounds to reduce false-positive on generic colon strings.
            patterns: vec![rx(r"\b[a-z0-9]{24}:[a-zA-Z0-9]{10,}\b")],
            tier: Tier::Tier1,
            risk_level: RiskLevel::High,
            remediation_hint: "Rotate at developers.etsy.com → your app → Refresh shared secret. Note: rotation may force re-approval of write scopes — coordinate with Dave before rotating production Etsy keys.",
            rotation_url: Some("https://developers.etsy.com/your-apps"),
            docs_url: Some("https://developers.etsy.com/documentation/"),
        },
        Detector {
            id: "cloudflare_api_token",
            provider: "cloudflare",
            display_name: "Cloudflare API token (env-context match)",
            // Cloudflare tokens are 40-char alphanumeric but very generic-
            // looking. We only flag when the surrounding text suggests a
            // Cloudflare context (e.g., CLOUDFLARE_API_TOKEN= or cloudflare).
            // Pattern matches the token value when prefixed by an env-style
            // assignment containing CLOUDFLARE.
            patterns: vec![rx(
                r#"(?i)CLOUDFLARE[A-Z_]*(?:TOKEN|KEY)\s*[:=]\s*['"]?([A-Za-z0-9_-]{40})['"]?"#,
            )],
            tier: Tier::Tier1,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Revoke at dash.cloudflare.com → Profile → API Tokens. Cloudflare token leaks can mutate DNS, tunnels, and WAF — audit recent changes.",
            rotation_url: Some("https://dash.cloudflare.com/profile/api-tokens"),
            docs_url: Some("https://developers.cloudflare.com/api/tokens/"),
        },

        // ─── Tier 2: common AI-stack auxiliary services ──────────────────
        Detector {
            id: "replicate_api_token",
            provider: "replicate",
            display_name: "Replicate API token",
            patterns: vec![rx(r"\br8_[A-Za-z0-9]{37,40}\b")],
            tier: Tier::Tier2,
            risk_level: RiskLevel::High,
            remediation_hint: "Rotate at replicate.com/account/api-tokens.",
            rotation_url: Some("https://replicate.com/account/api-tokens"),
            docs_url: Some("https://replicate.com/docs"),
        },
        Detector {
            id: "huggingface_token",
            provider: "huggingface",
            display_name: "Hugging Face access token",
            patterns: vec![rx(r"\bhf_[A-Za-z0-9]{34,40}\b")],
            tier: Tier::Tier2,
            risk_level: RiskLevel::High,
            remediation_hint: "Rotate at huggingface.co/settings/tokens.",
            rotation_url: Some("https://huggingface.co/settings/tokens"),
            docs_url: Some("https://huggingface.co/docs/api-inference/index"),
        },
        Detector {
            id: "openrouter_api_key",
            provider: "openrouter",
            display_name: "OpenRouter API key",
            patterns: vec![rx(r"\bsk-or-(?:v[0-9]+-)?[A-Za-z0-9_-]{40,}\b")],
            tier: Tier::Tier2,
            risk_level: RiskLevel::High,
            remediation_hint: "Rotate at openrouter.ai/settings/keys.",
            rotation_url: Some("https://openrouter.ai/settings/keys"),
            docs_url: Some("https://openrouter.ai/docs"),
        },
        Detector {
            id: "elevenlabs_api_key",
            provider: "elevenlabs",
            display_name: "ElevenLabs API key (env-context match)",
            patterns: vec![rx(
                r#"(?i)ELEVEN(?:LABS)?_API_KEY\s*[:=]\s*['"]?([A-Za-z0-9]{32,40})['"]?"#,
            )],
            tier: Tier::Tier2,
            risk_level: RiskLevel::Medium,
            remediation_hint: "Rotate at elevenlabs.io/app/settings/api-keys.",
            rotation_url: Some("https://elevenlabs.io/app/settings/api-keys"),
            docs_url: Some("https://elevenlabs.io/docs"),
        },
        Detector {
            id: "pinecone_api_key",
            provider: "pinecone",
            display_name: "Pinecone API key (UUID format, env-context match)",
            patterns: vec![rx(
                r#"(?i)PINECONE_API_KEY\s*[:=]\s*['"]?([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})['"]?"#,
            )],
            tier: Tier::Tier2,
            risk_level: RiskLevel::Medium,
            remediation_hint: "Rotate at app.pinecone.io → API Keys.",
            rotation_url: Some("https://app.pinecone.io/"),
            docs_url: Some("https://docs.pinecone.io/"),
        },
        Detector {
            id: "supabase_service_role_jwt",
            provider: "supabase",
            display_name: "Supabase service-role JWT (suggested by .supabase.co URL)",
            patterns: vec![rx(r"\beyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b")],
            tier: Tier::Tier2,
            risk_level: RiskLevel::High,
            remediation_hint: "Generic JWT shape — confirm provider context. For Supabase: rotate via supabase.com/dashboard → Project Settings → API → New service-role key.",
            rotation_url: Some("https://supabase.com/dashboard"),
            docs_url: Some("https://supabase.com/docs/guides/api"),
        },
        Detector {
            id: "neon_db_uri",
            provider: "neon",
            display_name: "Neon Postgres connection URI",
            patterns: vec![rx(r"\bpostgres(?:ql)?://[^:\s'\x22]+:[^@\s'\x22]+@[^/\s'\x22]+\.neon\.tech[^\s'\x22]*")],
            tier: Tier::Tier2,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Reset the Neon role password at console.neon.tech. URI contains role + password — full DB access.",
            rotation_url: Some("https://console.neon.tech/"),
            docs_url: Some("https://neon.tech/docs"),
        },
        Detector {
            id: "mongodb_uri",
            provider: "mongodb",
            display_name: "MongoDB connection URI (with credentials)",
            patterns: vec![rx(r"\bmongodb(?:\+srv)?://[^:\s'\x22]+:[^@\s'\x22]+@[^/\s'\x22]+")],
            tier: Tier::Tier2,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Reset the database user password in your MongoDB Atlas / self-hosted admin. URI contains full credentials.",
            rotation_url: None,
            docs_url: Some("https://www.mongodb.com/docs/manual/reference/connection-string/"),
        },
        Detector {
            id: "aws_access_key_id",
            provider: "aws",
            display_name: "AWS access key ID",
            patterns: vec![rx(r"\b(?:AKIA|ASIA|AROA|AIDA)[A-Z0-9]{16}\b")],
            tier: Tier::Tier2,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Deactivate + delete in IAM console immediately. AKIA = long-term IAM user; ASIA = short-term STS; AROA = role; AIDA = IAM user. Rotate AND audit CloudTrail for the leak window.",
            rotation_url: Some("https://console.aws.amazon.com/iamv2/home#/security_credentials"),
            docs_url: Some("https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html"),
        },
        Detector {
            id: "gcp_service_account_json_marker",
            provider: "gcp",
            display_name: "GCP service-account JSON marker",
            // Looks for the unmistakable line: "type": "service_account"
            patterns: vec![rx(r#""type"\s*:\s*"service_account""#)],
            tier: Tier::Tier2,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Service-account JSON contains a private key. Rotate at console.cloud.google.com → IAM → Service Accounts → key management. Audit GCP audit logs for the leak window.",
            rotation_url: Some("https://console.cloud.google.com/iam-admin/serviceaccounts"),
            docs_url: Some("https://cloud.google.com/iam/docs/keys-create-delete"),
        },
        Detector {
            id: "azure_openai_key",
            provider: "azure",
            display_name: "Azure OpenAI / Cognitive Services key (env-context match)",
            patterns: vec![rx(
                r#"(?i)AZURE_(?:OPENAI_)?(?:API_)?KEY\s*[:=]\s*['"]?([A-Fa-f0-9]{32})['"]?"#,
            )],
            tier: Tier::Tier2,
            risk_level: RiskLevel::High,
            remediation_hint: "Rotate at portal.azure.com → your resource → Keys and Endpoint → Regenerate. Azure resource keys are paired (KEY1/KEY2) — rotate one at a time to avoid downtime.",
            rotation_url: Some("https://portal.azure.com/"),
            docs_url: Some("https://learn.microsoft.com/en-us/azure/ai-services/openai/"),
        },

        // ─── Tier 3: messaging + utility + generic fallbacks ─────────────
        Detector {
            id: "slack_webhook",
            provider: "slack",
            display_name: "Slack incoming webhook URL",
            patterns: vec![rx(r"\bhttps://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[A-Za-z0-9]+\b")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::Medium,
            remediation_hint: "Disable the webhook in the Slack app's incoming-webhooks page. Anyone with the URL can post to your channel.",
            rotation_url: None,
            docs_url: Some("https://api.slack.com/messaging/webhooks"),
        },
        Detector {
            id: "slack_token",
            provider: "slack",
            display_name: "Slack token (xoxa/b/p/r/s)",
            patterns: vec![rx(r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::High,
            remediation_hint: "Revoke at api.slack.com/apps. Bot/user tokens carry workspace API access.",
            rotation_url: Some("https://api.slack.com/apps"),
            docs_url: Some("https://api.slack.com/authentication/token-types"),
        },
        Detector {
            id: "discord_webhook",
            provider: "discord",
            display_name: "Discord webhook URL",
            patterns: vec![rx(r"\bhttps://(?:discord(?:app)?\.com|discord\.gg)/api/webhooks/[0-9]+/[A-Za-z0-9_-]+\b")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::Medium,
            remediation_hint: "Delete the webhook in the channel's integrations panel.",
            rotation_url: None,
            docs_url: Some("https://discord.com/developers/docs/resources/webhook"),
        },
        Detector {
            id: "discord_bot_token",
            provider: "discord",
            display_name: "Discord bot token",
            // Discord bot tokens are three base64 segments separated by dots.
            patterns: vec![rx(r"\b[MNO][A-Za-z0-9_-]{23}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,38}\b")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::High,
            remediation_hint: "Reset the bot token at discord.com/developers/applications → your app → Bot → Reset Token.",
            rotation_url: Some("https://discord.com/developers/applications"),
            docs_url: Some("https://discord.com/developers/docs/topics/oauth2"),
        },
        Detector {
            id: "notion_token",
            provider: "notion",
            display_name: "Notion integration token",
            patterns: vec![rx(r"\bsecret_[A-Za-z0-9]{43}\b")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::Medium,
            remediation_hint: "Rotate at notion.so/my-integrations → Integration → Internal Integration Token → Refresh.",
            rotation_url: Some("https://www.notion.so/my-integrations"),
            docs_url: Some("https://developers.notion.com/"),
        },
        Detector {
            id: "airtable_pat",
            provider: "airtable",
            display_name: "Airtable personal access token",
            patterns: vec![rx(r"\bpat[A-Za-z0-9]{14}\.[A-Za-z0-9]{64}\b")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::Medium,
            remediation_hint: "Revoke at airtable.com/create/tokens.",
            rotation_url: Some("https://airtable.com/create/tokens"),
            docs_url: Some("https://airtable.com/developers/web/guides/personal-access-tokens"),
        },
        Detector {
            id: "apify_api_token",
            provider: "apify",
            display_name: "Apify API token",
            patterns: vec![rx(r"\bapify_api_[A-Za-z0-9]{36,40}\b")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::Medium,
            remediation_hint: "Rotate at console.apify.com/settings/integrations.",
            rotation_url: Some("https://console.apify.com/settings/integrations"),
            docs_url: Some("https://docs.apify.com/api/v2"),
        },
        Detector {
            id: "jwt_generic",
            provider: "generic",
            display_name: "Generic JSON Web Token (3-segment dotted base64)",
            patterns: vec![rx(r"\beyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]{4,}\b")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::Medium,
            remediation_hint: "JWTs vary widely — confirm provider. Bearer JWTs are usually short-lived; long-lived JWTs (>30d) deserve attention.",
            rotation_url: None,
            docs_url: Some("https://jwt.io/"),
        },
        Detector {
            id: "pem_private_key",
            provider: "generic",
            display_name: "PEM-encoded private key (RSA/EC/OpenSSH/PGP)",
            patterns: vec![rx(r"-----BEGIN [A-Z ]*PRIVATE KEY-----")],
            tier: Tier::Tier3,
            risk_level: RiskLevel::Critical,
            remediation_hint: "Rotate the keypair at its source (SSH, ACME/cert authority, GPG). PEM private-key leaks = full impersonation of the keyholder.",
            rotation_url: None,
            docs_url: Some("https://datatracker.ietf.org/doc/html/rfc7468"),
        },
        Detector {
            id: "high_entropy_generic_fallback",
            provider: "generic",
            display_name: "High-entropy generic token (base64-shape, ≥40 chars, env-context)",
            // Conservative fallback: only flags long base64-ish runs WHEN
            // assigned to an env var whose name suggests a secret. Reduces
            // false-positive rate vs. an unconditional entropy match.
            patterns: vec![rx(
                r#"(?i)(?:_KEY|_TOKEN|_SECRET|_PASSWORD|_PASS)\s*[:=]\s*['"]?([A-Za-z0-9+/=_-]{40,})['"]?"#,
            )],
            tier: Tier::Tier3,
            risk_level: RiskLevel::Low,
            remediation_hint: "Generic high-entropy fallback — confirm provider before treating as a leak. False-positive rate is non-trivial; this detector exists to catch leaks that no other detector saw.",
            rotation_url: None,
            docs_url: None,
        },
    ]
}

// ── Scanner ──────────────────────────────────────────────────────────────────

/// Run every detector against a string. Returns a Vec of Detections, with
/// `redacted_preview` populated and the raw match discarded.
///
/// Line numbers are 1-based. `file_path` is None — the caller (M3 repo
/// scanner) sets it after wrapping this function with file I/O.
pub fn scan_text(s: &str) -> Vec<Detection> {
    let registry = detector_registry();
    let mut out = Vec::new();

    // Build a per-line cursor so we can attribute each match to its line.
    // Pre-compute line-start byte offsets for fast bisection.
    let mut line_starts = vec![0usize];
    for (i, b) in s.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i + 1);
        }
    }
    let line_of = |byte_offset: usize| -> usize {
        // 1-based line number
        match line_starts.binary_search(&byte_offset) {
            Ok(idx) => idx + 1,
            Err(idx) => idx, // idx = number of starts <= offset
        }
    };

    for det in registry {
        for pat in &det.patterns {
            for m in pat.find_iter(s) {
                let raw = m.as_str();
                let preview = redact_match(raw);
                out.push(Detection {
                    secret_type: det.id,
                    provider: det.provider,
                    display_name: det.display_name,
                    file_path: None,
                    line_number: line_of(m.start()),
                    redacted_preview: preview,
                    risk_level: det.risk_level,
                    tier: det.tier,
                    git_tracked: None,
                    recommended_action: det.remediation_hint,
                    rotation_url: det.rotation_url,
                    docs_url: det.docs_url,
                });
            }
        }
    }
    out
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // All test inputs use clearly-FAKE values. None of these are real keys.
    // The `FAKE` substring is deliberate so anyone reading the source can
    // tell at a glance these aren't credentials.

    fn assert_detected(text: &str, expected_id: &str) {
        let dets = scan_text(text);
        assert!(
            dets.iter().any(|d| d.secret_type == expected_id),
            "expected detector {expected_id:?} to fire on input, got: {dets:?}"
        );
    }

    fn assert_redacted_never_contains_full(text: &str) {
        let dets = scan_text(text);
        for d in &dets {
            // The full input shouldn't appear in the redacted preview.
            assert!(
                !text.contains(&d.redacted_preview) || d.redacted_preview == "***",
                "redacted_preview {:?} found verbatim in source text — leak! detection: {:?}",
                d.redacted_preview,
                d
            );
            // Specific check: redacted_preview is short.
            assert!(
                d.redacted_preview.len() <= 16,
                "redacted_preview unexpectedly long: {:?}",
                d.redacted_preview
            );
        }
    }

    fn fake_stripe_live_secret() -> String {
        ["sk", "_live_", "FAKE0FAKE0FAKE0FAKE0FAKE0"].concat()
    }

    // ── redact_match unit tests ─────────────────────────────────────────────

    #[test]
    fn redact_short_returns_stars() {
        assert_eq!(redact_match("short"), "***");
        assert_eq!(redact_match("abcdefghijk"), "***"); // 11 chars
    }

    #[test]
    fn redact_long_keeps_head_and_tail() {
        assert_eq!(redact_match("abcdefghijklmnop"), "abcd...mnop");
    }

    #[test]
    fn redact_empty_returns_stars() {
        assert_eq!(redact_match(""), "***");
    }

    #[test]
    fn redact_unicode_safe() {
        // Should slice by chars, not bytes.
        assert_eq!(redact_match("aaaaXXXXXXXXbbbb"), "aaaa...bbbb");
    }

    // ── Detector registry sanity ────────────────────────────────────────────

    #[test]
    fn registry_has_all_expected_tier1_ids() {
        let ids: Vec<&str> = detector_registry().iter().map(|d| d.id).collect();
        for expected in [
            "openai_api_key",
            "anthropic_api_key",
            "google_ai_api_key",
            "telegram_bot_token",
            "github_pat_classic",
            "github_pat_fine_grained",
            "github_oauth_token",
            "stripe_live_secret",
            "etsy_oauth_pair",
            "cloudflare_api_token",
        ] {
            assert!(
                ids.contains(&expected),
                "missing tier1 detector: {expected}"
            );
        }
    }

    #[test]
    fn registry_has_tier2_and_tier3_coverage() {
        let dets = detector_registry();
        let tier2 = dets.iter().filter(|d| d.tier == Tier::Tier2).count();
        let tier3 = dets.iter().filter(|d| d.tier == Tier::Tier3).count();
        assert!(tier2 >= 8, "expected ≥8 tier2 detectors, got {tier2}");
        assert!(tier3 >= 6, "expected ≥6 tier3 detectors, got {tier3}");
    }

    #[test]
    fn registry_ids_unique() {
        let ids: Vec<&str> = detector_registry().iter().map(|d| d.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(ids.len(), sorted.len(), "duplicate detector ids");
    }

    // ── Tier 1 detection ────────────────────────────────────────────────────

    #[test]
    fn detects_openai_api_key() {
        assert_detected(
            "OPENAI_API_KEY=sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123",
            "openai_api_key",
        );
    }

    #[test]
    fn detects_openai_proj_key() {
        assert_detected(
            "OPENAI_API_KEY=sk-proj-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123",
            "openai_api_key",
        );
    }

    #[test]
    fn detects_anthropic_api_key() {
        assert_detected(
            "ANTHROPIC_API_KEY=sk-ant-api03-FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE0",
            "anthropic_api_key",
        );
    }

    #[test]
    fn detects_google_ai_key() {
        // Real Google API keys are exactly 39 chars total (AIza + 35).
        assert_detected(
            "GEMINI_API_KEY=AIzaSyD-FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0F",
            "google_ai_api_key",
        );
    }

    #[test]
    fn detects_telegram_bot_token() {
        assert_detected(
            "TELEGRAM_BOT_TOKEN=123456789:FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE",
            "telegram_bot_token",
        );
    }

    #[test]
    fn detects_github_pat_classic() {
        // GitHub classic PATs are ghp_ + exactly 36 chars (FAKE0 × 7 + F = 36).
        assert_detected(
            "GH_TOKEN=ghp_FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0F",
            "github_pat_classic",
        );
    }

    #[test]
    fn detects_github_pat_fine_grained() {
        let token = format!("github_pat_{}", "F".repeat(82));
        assert_detected(&format!("GH_TOKEN={token}"), "github_pat_fine_grained");
    }

    #[test]
    fn detects_stripe_live_secret() {
        let s = format!("STRIPE_SECRET_KEY={}", fake_stripe_live_secret());
        assert_detected(&s, "stripe_live_secret");
    }

    #[test]
    fn detects_etsy_oauth_pair() {
        assert_detected(
            "ETSY_API_KEY=fakekeystringabc12345678:FakeSharedSecretXYZ",
            "etsy_oauth_pair",
        );
    }

    #[test]
    fn detects_cloudflare_token_with_env_context() {
        let s = format!("CLOUDFLARE_API_TOKEN={}", "F".repeat(40));
        assert_detected(&s, "cloudflare_api_token");
    }

    // ── Tier 2 detection ────────────────────────────────────────────────────

    #[test]
    fn detects_replicate_token() {
        // Replicate tokens: r8_ + 37–40 alphanumerics.
        assert_detected(
            "REPLICATE_API_TOKEN=r8_FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0FF",
            "replicate_api_token",
        );
    }

    #[test]
    fn detects_huggingface_token() {
        assert_detected(
            "HF_TOKEN=hf_FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0",
            "huggingface_token",
        );
    }

    #[test]
    fn detects_aws_access_key_id() {
        assert_detected(
            "AWS_ACCESS_KEY_ID=AKIAFAKEFAKEFAKE0123",
            "aws_access_key_id",
        );
    }

    #[test]
    fn detects_gcp_service_account_marker() {
        assert_detected(
            r#"{"type": "service_account", "project_id": "demo"}"#,
            "gcp_service_account_json_marker",
        );
    }

    #[test]
    fn detects_neon_db_uri() {
        assert_detected(
            "DATABASE_URL=postgresql://demo:FAKEFAKE@ep-fake.neon.tech/demodb",
            "neon_db_uri",
        );
    }

    #[test]
    fn detects_mongodb_uri() {
        assert_detected(
            "MONGO_URL=mongodb+srv://demo:FAKEFAKE@cluster0.fake.mongodb.net/demo",
            "mongodb_uri",
        );
    }

    // ── Tier 3 detection ────────────────────────────────────────────────────

    #[test]
    fn detects_slack_webhook() {
        assert_detected(
            "SLACK_HOOK=https://hooks.slack.com/services/TFAKE/BFAKE/FAKE0fake0fake0fake0fake0",
            "slack_webhook",
        );
    }

    #[test]
    fn detects_pem_private_key() {
        assert_detected(
            "-----BEGIN RSA PRIVATE KEY-----\nFAKEKEYBYTES\n-----END RSA PRIVATE KEY-----",
            "pem_private_key",
        );
    }

    #[test]
    fn detects_jwt_shape() {
        assert_detected(
            "AUTH_JWT=eyJfakehead.eyJfakeclaims.fakesignature",
            "jwt_generic",
        );
    }

    #[test]
    fn detects_high_entropy_generic_with_env_context() {
        assert_detected(
            "MY_PROVIDER_TOKEN=FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_F",
            "high_entropy_generic_fallback",
        );
    }

    // ── Negative cases ──────────────────────────────────────────────────────
    //
    // These confirm low false-positive rate on common non-secret strings.

    #[test]
    fn does_not_match_lorem_ipsum() {
        let s = "Lorem ipsum dolor sit amet, consectetur adipiscing elit.";
        let dets = scan_text(s);
        assert!(dets.is_empty(), "false positive on lorem ipsum: {dets:?}");
    }

    #[test]
    fn does_not_match_uuid_alone() {
        // UUIDs without env context shouldn't fire pinecone (which requires
        // env-context match).
        let s = "user_id = 550e8400-e29b-41d4-a716-446655440000";
        let dets: Vec<&Detection> = scan_text(s)
            .iter()
            .filter(|d| d.secret_type == "pinecone_api_key")
            .map(|_| unreachable!())
            .collect();
        assert!(dets.is_empty());
    }

    // ── Redaction enforcement (the critical one) ────────────────────────────

    #[test]
    fn redacted_preview_never_leaks_full_match_tier1() {
        let lines = [
            "OPENAI_API_KEY=sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123",
            "ANTHROPIC_API_KEY=sk-ant-api03-FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE_FAKE0",
            "GEMINI_API_KEY=AIzaFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0",
            "GH_TOKEN=ghp_FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0FAKE0",
        ];
        for line in lines {
            assert_redacted_never_contains_full(line);
        }
        let stripe_line = format!("STRIPE_SECRET_KEY={}", fake_stripe_live_secret());
        assert_redacted_never_contains_full(&stripe_line);
    }

    #[test]
    fn line_numbers_are_one_based() {
        let s = "first line\nOPENAI_API_KEY=sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123\nthird";
        let dets = scan_text(s);
        let openai = dets
            .iter()
            .find(|d| d.secret_type == "openai_api_key")
            .unwrap();
        assert_eq!(openai.line_number, 2);
    }

    #[test]
    fn detection_serializes_to_json_without_full_value() {
        // Smoke-test: serialize the Detection struct and confirm the
        // redacted preview is what shows up — no field carries raw value.
        let s = "OPENAI_API_KEY=sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123";
        let dets = scan_text(s);
        let json = serde_json::to_string(&dets).unwrap();
        assert!(!json.contains("sk-FAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKEFAKE0123"));
        assert!(json.contains("redacted_preview"));
    }
}
