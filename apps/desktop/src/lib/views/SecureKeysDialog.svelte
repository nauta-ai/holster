<script lang="ts">
  import {
    listKeys,
    copyToClipboard,
    retryMirrorKey,
    type KeyMetadataDto
  } from '$lib/api';
  import AddKeyDialog from './AddKeyDialog.svelte';
  import ConfirmDelete from './ConfirmDelete.svelte';

  interface Props {
    onClose: () => void;
    onSessionExpired: () => void;
    onToast: (message: string) => void;
  }
  let { onClose, onSessionExpired, onToast }: Props = $props();

  let keys = $state<KeyMetadataDto[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let filter = $state('');
  let showAdd = $state(false);
  let confirmTarget = $state<KeyMetadataDto | null>(null);
  let retryingMirrorId = $state<string | null>(null);

  function handleSessionError(msg: string) {
    const lower = msg.toLowerCase();
    if (lower.includes('session expired') || lower.includes('session is invalid')) {
      onSessionExpired();
      return true;
    }
    return false;
  }

  async function refresh() {
    loading = true;
    error = null;
    try {
      keys = await listKeys();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (handleSessionError(msg)) return;
      error = msg;
    } finally {
      loading = false;
    }
  }

  async function handleCopy(key: KeyMetadataDto) {
    try {
      await copyToClipboard(key.id);
      onToast(`Copied ${key.label}. Clipboard clears in 30 sec.`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (handleSessionError(msg)) return;
      onToast(`Copy failed: ${msg}`);
    }
  }

  async function handleRetryMirror(key: KeyMetadataDto) {
    retryingMirrorId = key.id;
    try {
      await retryMirrorKey(key.id);
      onToast(`Runtime mirror synced for ${key.label}.`);
      await refresh();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (handleSessionError(msg)) return;
      onToast(`Runtime mirror still failed: ${msg}`);
    } finally {
      retryingMirrorId = null;
    }
  }

  const filteredKeys = $derived.by(() => {
    const f = filter.trim().toLowerCase();
    if (!f) return keys;
    return keys.filter((k) => {
      return (
        k.label.toLowerCase().includes(f) ||
        k.provider.toLowerCase().includes(f) ||
        (k.project_tag ?? '').toLowerCase().includes(f)
      );
    });
  });

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !showAdd && !confirmTarget) onClose();
  }

  function formatDate(iso: string | null): string {
    if (!iso) return '—';
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  }

  refresh();
</script>

<svelte:window onkeydown={onKeydown} />

<div class="modal-backdrop" role="presentation" onclick={onBackdropClick}>
  <div class="modal vault-modal" role="dialog" aria-modal="true" aria-labelledby="vault-title">
    <header class="dialog-header">
      <div>
        <p class="eyebrow">Encrypted vault</p>
        <h2 id="vault-title">Secure API Keys</h2>
        <p class="dialog-subtitle">
          Keys live in the SQLCipher vault on disk. Values never appear in the UI.
          Copy auto-clears the clipboard in 30 seconds.
        </p>
      </div>
      <button type="button" class="dialog-close" onclick={onClose} aria-label="Close">×</button>
    </header>

    <section class="vault-toolbar">
      <input
        type="search"
        bind:value={filter}
        placeholder="Filter by provider, label, or project"
        class="vault-filter"
        aria-label="Filter keys"
      />
      <button type="button" class="vault-add" onclick={() => (showAdd = true)}>
        + Add key
      </button>
    </section>

    {#if error}
      <div class="error-box" role="alert">
        <strong>Could not load keys.</strong>
        <p>{error}</p>
        <button type="button" onclick={refresh}>Try again</button>
      </div>
    {/if}

    {#if loading}
      <p class="vault-loading">Loading vault…</p>
    {:else if keys.length === 0}
      <div class="vault-empty">
        <p class="vault-empty-title">Vault is empty.</p>
        <p>
          Add the API keys you use so Holster can generate matching
          <code>.env.example</code> placeholders and agent runtime profiles.
        </p>
        <button type="button" class="vault-add" onclick={() => (showAdd = true)}>
          + Add your first key
        </button>
      </div>
    {:else if filteredKeys.length === 0}
      <p class="vault-loading">No keys match “{filter}”.</p>
    {:else}
      <p class="vault-count">
        {filteredKeys.length} of {keys.length} key{keys.length === 1 ? '' : 's'} shown
      </p>
      <ul class="vault-list">
        {#each filteredKeys as key}
          <li class="vault-row">
            <div class="vault-row-meta">
              <span class="vault-provider">{key.provider}</span>
              <div class="vault-info">
                <p class="vault-label">{key.label}</p>
                <p class="vault-sub">
                  {#if key.project_tag}
                    <span>{key.project_tag}</span>
                    <span class="vault-dot">·</span>
                  {/if}
                  <span>added {formatDate(key.created_at)}</span>
                  {#if key.last_used_at}
                    <span class="vault-dot">·</span>
                    <span>last used {formatDate(key.last_used_at)}</span>
                  {/if}
                </p>
                {#if key.mirror_failed}
                  <div class="mirror-warning" title={key.mirror_error ?? 'Mirror failed'}>
                    <span aria-hidden="true">⚠</span>
                    <span>Not mirrored to runtime vault.</span>
                    <button
                      type="button"
                      onclick={() => handleRetryMirror(key)}
                      disabled={retryingMirrorId === key.id}
                    >
                      {retryingMirrorId === key.id ? 'Retrying…' : 'Retry sync'}
                    </button>
                  </div>
                {/if}
              </div>
            </div>
            <div class="vault-actions">
              <button type="button" onclick={() => handleCopy(key)}>Copy</button>
              <button
                type="button"
                class="vault-danger"
                onclick={() => (confirmTarget = key)}
              >
                Delete
              </button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>

{#if showAdd}
  <AddKeyDialog
    onClose={() => (showAdd = false)}
    onAdded={(key) => {
      showAdd = false;
      refresh();
      if (key.mirror_failed) {
        onToast('Key added. Runtime mirror needs retry before LaunchAgents can see it.');
      } else {
        onToast('Key added to the vault and mirrored to runtime.');
      }
    }}
  />
{/if}

{#if confirmTarget}
  <ConfirmDelete
    target={confirmTarget}
    onClose={() => (confirmTarget = null)}
    onDeleted={() => {
      const label = confirmTarget?.label ?? 'key';
      confirmTarget = null;
      refresh();
      onToast(`Deleted ${label}.`);
    }}
  />
{/if}

<style>
  .vault-modal {
    max-width: 880px;
    width: 100%;
    max-height: 90vh;
    overflow-y: auto;
    padding: var(--spacing-xl, 24px);
  }

  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--spacing-md, 12px);
    margin-bottom: var(--spacing-lg, 18px);
  }

  .dialog-header h2 {
    margin: 4px 0 6px;
    font-size: 22px;
    color: #ffffff;
  }

  .dialog-subtitle {
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.5;
    margin: 0;
    max-width: 60ch;
  }

  .dialog-close {
    background: transparent;
    border: none;
    font-size: 24px;
    line-height: 1;
    color: rgba(255, 255, 255, 0.55);
    cursor: pointer;
    padding: 4px 10px;
    border-radius: 6px;
    transition: background 120ms ease, color 120ms ease;
  }

  .dialog-close:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .vault-toolbar {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-bottom: 16px;
  }

  .vault-filter {
    flex: 1;
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.04);
    color: #e8edf6;
    font: inherit;
  }

  .vault-filter::placeholder {
    color: rgba(255, 255, 255, 0.4);
  }

  .vault-add {
    background: var(--accent, #b8781f);
    color: #1a1a1f;
    border: 1px solid var(--accent, #b8781f);
    border-radius: 8px;
    padding: 8px 14px;
    font-weight: 600;
    cursor: pointer;
    transition: filter 120ms ease;
  }

  .vault-add:hover {
    filter: brightness(1.08);
  }

  .vault-loading,
  .vault-count {
    color: rgba(255, 255, 255, 0.6);
    font-size: 13px;
    margin: 0 0 12px;
  }

  .vault-empty {
    padding: 32px;
    border: 1px dashed rgba(255, 255, 255, 0.15);
    border-radius: 12px;
    text-align: center;
    color: rgba(255, 255, 255, 0.7);
  }

  .vault-empty-title {
    color: #ffffff;
    font-size: 16px;
    font-weight: 600;
    margin: 0 0 6px;
  }

  .vault-empty button {
    margin-top: 16px;
  }

  .vault-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .vault-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 12px 14px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
  }

  .vault-row-meta {
    display: flex;
    gap: 14px;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  .vault-provider {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.7);
    background: rgba(255, 255, 255, 0.08);
    padding: 4px 8px;
    border-radius: 4px;
    flex-shrink: 0;
  }

  .vault-info {
    flex: 1;
    min-width: 0;
  }

  .vault-label {
    color: #ffffff;
    font-weight: 500;
    margin: 0;
    word-break: break-word;
  }

  .vault-sub {
    color: rgba(255, 255, 255, 0.55);
    font-size: 12px;
    margin: 4px 0 0;
  }

  .vault-dot {
    margin: 0 6px;
    opacity: 0.5;
  }

  .vault-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .vault-actions button {
    background: rgba(255, 255, 255, 0.04);
    color: #e8edf6;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 13px;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease;
  }

  .vault-actions button:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .vault-actions .vault-danger {
    color: #f3a18d;
    border-color: rgba(240, 130, 110, 0.35);
  }

  .vault-actions .vault-danger:hover {
    background: rgba(176, 74, 48, 0.15);
    border-color: rgba(240, 130, 110, 0.55);
  }

  .mirror-warning {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    color: #ffd166;
    font-size: 12px;
  }

  .mirror-warning button {
    background: rgba(255, 209, 102, 0.12);
    color: #ffd166;
    border: 1px solid rgba(255, 209, 102, 0.35);
    border-radius: 6px;
    padding: 3px 7px;
    font-size: 12px;
    cursor: pointer;
  }

  .mirror-warning button:hover {
    background: rgba(255, 209, 102, 0.18);
  }

  .mirror-warning button:disabled {
    opacity: 0.65;
    cursor: wait;
  }
</style>
