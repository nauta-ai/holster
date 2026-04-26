<script lang="ts">
  import { listKeys, lockVault, copyToClipboard, type KeyMetadataDto } from '$lib/api';
  import AddKeyDialog from './AddKeyDialog.svelte';
  import ConfirmDelete from './ConfirmDelete.svelte';

  interface Props {
    onLocked: () => void;
    onSessionExpired: () => void;
  }
  let { onLocked, onSessionExpired }: Props = $props();

  let keys = $state<KeyMetadataDto[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let toast = $state<string | null>(null);
  let showAdd = $state(false);
  let confirmTarget = $state<KeyMetadataDto | null>(null);

  async function refresh() {
    loading = true;
    error = null;
    try {
      keys = await listKeys();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // The vault crate signals SessionExpired via an "Session expired due to
      // inactivity." string from our err_to_string mapper. Pop the user back to
      // the unlock screen.
      if (msg.toLowerCase().includes('session expired')) {
        onSessionExpired();
        return;
      }
      if (msg.toLowerCase().includes('session is invalid')) {
        onSessionExpired();
        return;
      }
      error = msg;
    } finally {
      loading = false;
    }
  }

  async function onCopy(k: KeyMetadataDto) {
    try {
      const secs = await copyToClipboard(k.id);
      showToast(`Copied — clipboard auto-clears in ${secs}s`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.toLowerCase().includes('session expired') || msg.toLowerCase().includes('session is invalid')) {
        onSessionExpired();
        return;
      }
      error = msg;
    }
  }

  async function onLock() {
    try {
      await lockVault();
    } finally {
      onLocked();
    }
  }

  function showToast(msg: string) {
    toast = msg;
    setTimeout(() => {
      if (toast === msg) toast = null;
    }, 3000);
  }

  function fmtDate(s: string | null): string {
    if (!s) return '—';
    try {
      return new Date(s).toLocaleString();
    } catch {
      return s;
    }
  }

  // Poll list_keys every 60s — this serves as both a "refresh metadata" tick
  // (last_used_at can change) and a session-expiry probe. If the vault has
  // auto-locked due to idle, the next list_keys returns SessionExpired and
  // we'll bounce back to the unlock screen.
  let pollTimer: ReturnType<typeof setInterval> | undefined;
  $effect(() => {
    refresh();
    pollTimer = setInterval(refresh, 60_000);
    return () => {
      if (pollTimer) clearInterval(pollTimer);
    };
  });
</script>

<div class="app-shell">
  <header>
    <div class="brand">HOLSTER</div>
    <div class="actions">
      <button onclick={() => (showAdd = true)} class="primary">+ Add key</button>
      <button onclick={refresh} class="ghost" title="Refresh">↻</button>
      <button onclick={onLock} class="ghost">Lock</button>
    </div>
  </header>

  <main>
    {#if error}
      <div class="error-box">{error}</div>
    {/if}

    {#if loading && keys.length === 0}
      <div class="empty">Loading…</div>
    {:else if keys.length === 0}
      <div class="empty">
        <p>No keys yet.</p>
        <button class="primary" onclick={() => (showAdd = true)}>Add your first key</button>
      </div>
    {:else}
      <table class="keys-table">
        <thead>
          <tr>
            <th>Provider</th>
            <th>Label</th>
            <th>Project</th>
            <th>Created</th>
            <th>Last used</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each keys as k (k.id)}
            <tr>
              <td><span class="provider-badge">{k.provider}</span></td>
              <td>{k.label}</td>
              <td>{k.project_tag ?? '—'}</td>
              <td class="id-cell">{fmtDate(k.created_at)}</td>
              <td class="id-cell">{fmtDate(k.last_used_at)}</td>
              <td>
                <div class="row-actions">
                  <button onclick={() => onCopy(k)}>Copy</button>
                  <button class="danger" onclick={() => (confirmTarget = k)}>Delete</button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </main>
</div>

{#if showAdd}
  <AddKeyDialog
    onClose={() => (showAdd = false)}
    onAdded={() => {
      showAdd = false;
      refresh();
    }}
  />
{/if}

{#if confirmTarget}
  <ConfirmDelete
    target={confirmTarget}
    onClose={() => (confirmTarget = null)}
    onDeleted={() => {
      confirmTarget = null;
      refresh();
    }}
  />
{/if}

{#if toast}
  <div class="toast">{toast}</div>
{/if}
