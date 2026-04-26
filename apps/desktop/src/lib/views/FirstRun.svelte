<script lang="ts">
  import { createVault, unlockVault } from '$lib/api';

  interface Props {
    vaultPath: string | null;
    onCreated: () => void;
  }
  let { vaultPath, onCreated }: Props = $props();

  let password = $state('');
  let confirm = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = null;
    if (password.length < 8) {
      error = 'Password must be at least 8 characters.';
      return;
    }
    if (password !== confirm) {
      error = 'Passwords do not match.';
      return;
    }
    busy = true;
    try {
      await createVault(password);
      // Immediately unlock so we can drop straight into the main view.
      await unlockVault(password);
      // Clear password from memory (best-effort — JS strings are immutable).
      password = '';
      confirm = '';
      onCreated();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="center-screen">
  <div class="card">
    <h1>Welcome to Holster</h1>
    <p class="subtitle">No vault found. Create one to get started.</p>
    {#if vaultPath}
      <p class="muted" style="margin-bottom: 18px;">
        Vault will be created at:<br />
        <code style="font-size: 11px; word-break: break-all;">{vaultPath}</code>
      </p>
    {/if}
    <form onsubmit={submit}>
      <div class="field">
        <label for="pw1">Master password</label>
        <input id="pw1" type="password" bind:value={password} autocomplete="new-password" autofocus />
      </div>
      <div class="field">
        <label for="pw2">Confirm password</label>
        <input id="pw2" type="password" bind:value={confirm} autocomplete="new-password" />
      </div>
      {#if error}
        <div class="error-box">{error}</div>
      {/if}
      <button type="submit" class="primary" disabled={busy} style="width: 100%;">
        {busy ? 'Creating…' : 'Create vault'}
      </button>
      <p class="muted" style="margin-top: 14px; text-align: center;">
        Your password is the only way to decrypt this vault. There is no recovery.
      </p>
    </form>
  </div>
</div>
