<script lang="ts">
  import { unlockVault } from '$lib/api';

  interface Props {
    vaultPath: string | null;
    onUnlocked: () => void;
    /** When true, the user got here because their session timed out. */
    fromTimeout?: boolean;
  }
  let { vaultPath, onUnlocked, fromTimeout = false }: Props = $props();

  let password = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = null;
    busy = true;
    try {
      await unlockVault(password);
      password = '';
      onUnlocked();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="center-screen">
  <div class="card">
    <h1>Unlock Holster</h1>
    <p class="subtitle">
      {#if fromTimeout}Session expired. Re-enter your master password.{:else}Enter your master password to continue.{/if}
    </p>
    {#if vaultPath}
      <p class="muted" style="margin-bottom: 18px;">
        <code style="font-size: 11px; word-break: break-all;">{vaultPath}</code>
      </p>
    {/if}
    <form onsubmit={submit}>
      <div class="field">
        <label for="pw">Master password</label>
        <input id="pw" type="password" bind:value={password} autocomplete="current-password" autofocus />
      </div>
      {#if error}
        <div class="error-box">{error}</div>
      {/if}
      <button type="submit" class="primary" disabled={busy} style="width: 100%;">
        {busy ? 'Unlocking…' : 'Unlock'}
      </button>
    </form>
  </div>
</div>
