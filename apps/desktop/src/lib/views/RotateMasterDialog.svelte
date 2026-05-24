<!--
  v0.7.0: Rotate-master dialog (the v0.6.0 product gap that triggered the
  2026-05-24 credential-rotation cascade — closes it on the desktop side).

  Flow:
    1. User enters OLD master + NEW master + NEW master confirm.
    2. Client-side guards: new must differ from old, both new must match,
       new must be >= 8 chars (server also enforces).
    3. Calls Tauri rotate_master command — re-encrypts every entry +
       regenerates salt atomically.
    4. Success: shows "Rotated N entries" + tells the user the vault is
       now LOCKED (rotation invalidates the session). They re-unlock with
       the new password.

  Security notes:
    - Plaintext passwords live in local component state only, cleared after
      submit (success or failure).
    - The Tauri command drops the strings after passing them to the vault.
    - On wrong-old-password, error surfaces from the Rust side (BadPassword
      / SQLCipher NotADatabase, both rendered as a clean error string).
-->
<script lang="ts">
  import { rotateMaster } from '$lib/api';

  interface Props {
    onClose: () => void;
    onRotated: () => void;
  }
  let { onClose, onRotated }: Props = $props();

  let oldPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let error = $state<string | null>(null);
  let success = $state<string | null>(null);
  let busy = $state(false);

  function reset() {
    oldPassword = '';
    newPassword = '';
    confirmPassword = '';
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = null;
    success = null;

    // Client-side guards (server validates again, but fail fast for UX)
    if (newPassword !== confirmPassword) {
      error = 'New password and confirmation do not match';
      return;
    }
    if (newPassword.length < 8) {
      error = 'New password must be at least 8 characters';
      return;
    }
    if (newPassword === oldPassword) {
      error = 'New password must differ from the old password';
      return;
    }

    busy = true;
    try {
      const count = await rotateMaster(oldPassword, newPassword);
      success = `Rotated master for ${count} ${count === 1 ? 'entry' : 'entries'}. Vault is now locked — sign in with your new password.`;
      reset();
      // Brief pause so the user can read the success line, then close + signal parent.
      setTimeout(() => {
        onRotated();
      }, 1800);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function cancel() {
    reset();
    onClose();
  }
</script>

<div class="dialog-backdrop">
  <div class="dialog-card">
    <header>
      <h2>Rotate master password</h2>
      <button type="button" class="close" aria-label="Close" onclick={cancel}>×</button>
    </header>

    <p class="subtitle">
      Re-encrypts every entry in the vault under a new master password. The
      operation is atomic — if it fails, the vault remains intact under your
      current master. After rotation, you'll need to sign in with the new
      password.
    </p>

    <form onsubmit={submit}>
      <div class="field">
        <label for="old-pw">Current master password</label>
        <input
          id="old-pw"
          type="password"
          bind:value={oldPassword}
          autocomplete="current-password"
          required
          disabled={busy}
        />
      </div>

      <div class="field">
        <label for="new-pw">New master password</label>
        <input
          id="new-pw"
          type="password"
          bind:value={newPassword}
          autocomplete="new-password"
          minlength="8"
          required
          disabled={busy}
        />
        <p class="hint">Minimum 8 characters. Use 16+ with mixed case, digits, and symbols.</p>
      </div>

      <div class="field">
        <label for="confirm-pw">Confirm new password</label>
        <input
          id="confirm-pw"
          type="password"
          bind:value={confirmPassword}
          autocomplete="new-password"
          minlength="8"
          required
          disabled={busy}
        />
      </div>

      {#if error}
        <div class="error-box">{error}</div>
      {/if}
      {#if success}
        <div class="success-box">{success}</div>
      {/if}

      <div class="actions">
        <button type="button" class="secondary" onclick={cancel} disabled={busy}>
          Cancel
        </button>
        <button type="submit" class="primary" disabled={busy || !!success}>
          {busy ? 'Rotating…' : success ? 'Done' : 'Rotate master'}
        </button>
      </div>
    </form>
  </div>
</div>

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .dialog-card {
    background: var(--surface, #ffffff);
    border-radius: 12px;
    padding: 24px;
    max-width: 480px;
    width: calc(100% - 32px);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.2);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  header h2 {
    margin: 0;
    font-size: 18px;
  }
  .close {
    background: none;
    border: none;
    font-size: 24px;
    cursor: pointer;
    color: var(--text-muted, #6b7280);
    padding: 0 4px;
  }
  .subtitle {
    color: var(--text-muted, #6b7280);
    font-size: 13px;
    line-height: 1.5;
    margin: 0 0 16px;
  }
  .field {
    margin-bottom: 14px;
  }
  .field label {
    display: block;
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 6px;
  }
  .field input {
    width: 100%;
    padding: 8px 10px;
    border: 1px solid var(--border, #d1d5db);
    border-radius: 6px;
    font-size: 14px;
    box-sizing: border-box;
  }
  .hint {
    font-size: 11px;
    color: var(--text-muted, #6b7280);
    margin: 4px 0 0;
  }
  .error-box {
    background: #fee2e2;
    border: 1px solid #fca5a5;
    color: #991b1b;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 13px;
    margin: 8px 0;
  }
  .success-box {
    background: #dcfce7;
    border: 1px solid #86efac;
    color: #166534;
    padding: 10px 12px;
    border-radius: 6px;
    font-size: 13px;
    margin: 8px 0;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }
  .actions button {
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
  }
  .actions button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .secondary {
    background: transparent;
    border-color: var(--border, #d1d5db);
    color: var(--text, #111827);
  }
  .primary {
    background: var(--primary, #d97706);
    color: white;
  }
</style>
