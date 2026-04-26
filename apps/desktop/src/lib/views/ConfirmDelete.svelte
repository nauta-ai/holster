<script lang="ts">
  import { deleteKey, type KeyMetadataDto } from '$lib/api';

  interface Props {
    target: KeyMetadataDto;
    onClose: () => void;
    onDeleted: () => void;
  }
  let { target, onClose, onDeleted }: Props = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  async function confirm() {
    error = null;
    busy = true;
    try {
      await deleteKey(target.id);
      onDeleted();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="modal-backdrop" role="presentation" onclick={onBackdropClick}>
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="del-title">
    <h2 id="del-title">Delete key?</h2>
    <p style="margin: 8px 0 0;">
      This will permanently delete <strong>{target.label}</strong>
      <span class="provider-badge">{target.provider}</span>.
    </p>
    <p class="muted" style="margin: 6px 0 0;">This cannot be undone.</p>
    {#if error}
      <div class="error-box">{error}</div>
    {/if}
    <div class="modal-actions">
      <button type="button" class="ghost" onclick={onClose} disabled={busy}>Cancel</button>
      <button type="button" class="danger" onclick={confirm} disabled={busy}>
        {busy ? 'Deleting…' : 'Delete'}
      </button>
    </div>
  </div>
</div>
