<script lang="ts">
  import { addKey, PROVIDERS } from '$lib/api';

  interface Props {
    onClose: () => void;
    onAdded: () => void;
  }
  let { onClose, onAdded }: Props = $props();

  let provider = $state<string>('anthropic');
  let label = $state('');
  let project_tag = $state('');
  let notes = $state('');
  let key_value = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = null;
    if (!label.trim()) { error = 'Label is required.'; return; }
    if (!key_value) { error = 'Key value is required.'; return; }
    busy = true;
    try {
      await addKey({
        provider,
        label: label.trim(),
        project_tag: project_tag.trim() || null,
        notes: notes.trim() || null,
        key_value
      });
      // Best-effort clear before onAdded triggers a re-render.
      key_value = '';
      onAdded();
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
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="add-title">
    <h2 id="add-title">Add key</h2>
    <p class="muted" style="margin-bottom: 18px;">Plaintext is encrypted and never leaves the device.</p>
    <form onsubmit={submit}>
      <div class="field">
        <label for="prov">Provider</label>
        <select id="prov" bind:value={provider}>
          {#each PROVIDERS as p}
            <option value={p}>{p}</option>
          {/each}
        </select>
      </div>
      <div class="field">
        <label for="lab">Label</label>
        <input id="lab" type="text" bind:value={label} autofocus />
      </div>
      <div class="field">
        <label for="proj">Project (optional)</label>
        <input id="proj" type="text" bind:value={project_tag} />
      </div>
      <div class="field">
        <label for="kv">Key value</label>
        <input id="kv" type="password" bind:value={key_value} autocomplete="off" spellcheck="false" />
      </div>
      <div class="field">
        <label for="notes">Notes (optional)</label>
        <textarea id="notes" rows="2" bind:value={notes}></textarea>
      </div>
      {#if error}
        <div class="error-box">{error}</div>
      {/if}
      <div class="modal-actions">
        <button type="button" class="ghost" onclick={onClose}>Cancel</button>
        <button type="submit" class="primary" disabled={busy}>
          {busy ? 'Adding…' : 'Add key'}
        </button>
      </div>
    </form>
  </div>
</div>
