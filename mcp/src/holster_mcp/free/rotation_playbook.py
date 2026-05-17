from __future__ import annotations

from typing import Any


PLAYBOOKS: dict[str, dict[str, Any]] = {
    "github": {
        "estimated_minutes": 15,
        "docs": "https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/creating-a-personal-access-token",
        "steps": [
            "Create a replacement fine-grained token with the minimum repository and organization permissions needed.",
            "Update local environment variables, CI secrets, and agent configs that use the old token.",
            "Run the smallest safe validation command for each consumer.",
            "Revoke the exposed token in GitHub settings.",
            "Re-scan the repository and confirm the old token no longer appears.",
        ],
    },
    "gitlab": {
        "estimated_minutes": 15,
        "docs": "https://docs.gitlab.com/user/profile/personal_access_tokens/",
        "steps": [
            "Create a replacement personal, project, or group access token with the narrowest required scopes.",
            "Update CI/CD variables and local secret stores that reference the exposed token.",
            "Validate the affected GitLab API or git operation with the new token.",
            "Revoke the old token from GitLab token settings.",
            "Check recent audit events for unexpected use of the exposed token.",
        ],
    },
    "stripe": {
        "estimated_minutes": 20,
        "docs": "https://docs.stripe.com/keys",
        "steps": [
            "Create a replacement restricted key when possible; otherwise create a new secret key.",
            "Update application environment variables, webhook workers, and deployment secrets.",
            "Run a test-mode or read-only Stripe API validation before changing live traffic.",
            "Roll or revoke the old key in the Stripe Dashboard.",
            "Review recent API request logs for unexpected activity.",
        ],
    },
    "aws": {
        "estimated_minutes": 30,
        "docs": "https://docs.aws.amazon.com/IAM/latest/UserGuide/id_credentials_access-keys.html",
        "steps": [
            "Create a second access key for the same IAM user only if a role-based replacement is not available.",
            "Update the local profile, CI secret, or workload secret that used the exposed access key.",
            "Validate with a read-only AWS CLI call scoped to the workload.",
            "Deactivate the old access key and monitor for failed callers.",
            "Delete the old access key after validation is complete.",
        ],
    },
    "gcp": {
        "estimated_minutes": 30,
        "docs": "https://cloud.google.com/iam/docs/keys-create-delete",
        "steps": [
            "Prefer Workload Identity or short-lived credentials; create a replacement service account key only if required.",
            "Update Secret Manager, CI variables, or local credentials that reference the exposed key.",
            "Validate the affected Google Cloud API with the replacement credential.",
            "Disable or delete the exposed service account key.",
            "Review service account key usage and IAM audit logs.",
        ],
    },
    "openai": {
        "estimated_minutes": 10,
        "docs": "https://platform.openai.com/api-keys",
        "steps": [
            "Create a replacement project API key from the OpenAI dashboard.",
            "Update local environment variables, CI secrets, and application secret stores.",
            "Run a minimal API smoke test against the intended project.",
            "Delete the exposed API key from the dashboard.",
            "Review project usage for unexpected calls during the exposure window.",
        ],
    },
    "anthropic": {
        "estimated_minutes": 10,
        "docs": "https://docs.anthropic.com/en/api/admin-api/apikeys/list-api-keys",
        "steps": [
            "Create a replacement API key in the Anthropic Console.",
            "Update local environment variables and deployment secrets.",
            "Run a minimal model call or account-safe validation with the new key.",
            "Revoke the exposed key.",
            "Review usage for unexpected activity.",
        ],
    },
    "gemini": {
        "estimated_minutes": 15,
        "docs": "https://ai.google.dev/gemini-api/docs/api-key",
        "steps": [
            "Create a replacement Gemini API key in Google AI Studio or the linked Google Cloud project.",
            "Apply API restrictions if the key supports them for the project.",
            "Update local and deployment secrets that reference the exposed key.",
            "Run a minimal Gemini API validation.",
            "Delete the exposed key and review recent usage.",
        ],
    },
    "brave": {
        "estimated_minutes": 10,
        "docs": "https://api-dashboard.search.brave.com/app/documentation",
        "steps": [
            "Create or regenerate the Brave Search API key from the API dashboard.",
            "Update local environment variables and server-side secret stores.",
            "Run a minimal Brave Search API validation.",
            "Revoke or retire the old key if the dashboard exposes that control.",
            "Review usage and billing for unexpected search volume.",
        ],
    },
    "pinterest": {
        "estimated_minutes": 25,
        "docs": "https://developers.pinterest.com/docs/api/v5/",
        "steps": [
            "Create or rotate the app secret or OAuth token from the Pinterest developer dashboard.",
            "Update OAuth client configuration and application secret stores.",
            "Re-authorize affected OAuth clients if the token type requires user consent.",
            "Validate the smallest safe Pinterest API request.",
            "Revoke the exposed token or old app credential and review app activity.",
        ],
    },
}

COMMON_WARNINGS = [
    "Do not revoke the old credential until the replacement has been verified.",
    "Never paste the exposed credential into chat, tickets, or logs.",
    "Prefer the narrowest possible scope when creating the replacement.",
]


def rotation_playbook(provider: str) -> dict[str, Any]:
    key = provider.strip().lower()
    if key not in PLAYBOOKS:
        supported = ", ".join(sorted(PLAYBOOKS))
        raise ValueError(f"unsupported provider: {provider}; supported providers: {supported}")
    playbook = PLAYBOOKS[key]
    return {
        "ok": True,
        "provider": key,
        "steps": list(playbook["steps"]),
        "estimated_minutes": int(playbook["estimated_minutes"]),
        "warnings": [*COMMON_WARNINGS, f"Reference docs: {playbook['docs']}"],
    }
