<script lang="ts">
  import {
    addTotpAccount,
    getTotpCode,
    listTotpAccounts,
    type TotpAccountDto,
    type TotpCodeReport
  } from '$lib/api';

  interface Props {
    onClose: () => void;
    onSessionExpired: () => void;
    onToast: (message: string) => void;
  }

  let { onClose, onSessionExpired, onToast }: Props = $props();

  let accounts = $state<TotpAccountDto[]>([]);
  let selectedId = $state<string | null>(null);
  let codeReport = $state<TotpCodeReport | null>(null);
  let loading = $state(true);
  let saving = $state(false);
  let error = $state<string | null>(null);

  let label = $state('');
  let issuer = $state('');
  let accountName = $state('');
  let secretOrUri = $state('');
  let backupCodes = $state('');

  function handleSessionError(msg: string) {
    if (msg.toLowerCase().includes('session expired') || msg.toLowerCase().includes('session is invalid')) {
      onSessionExpired();
      return true;
    }
    return false;
  }

  async function refresh() {
    loading = true;
    error = null;
    try {
      accounts = await listTotpAccounts();
      if (!selectedId && accounts.length > 0) selectedId = accounts[0].id;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (handleSessionError(msg)) return;
      error = msg;
    } finally {
      loading = false;
    }
  }

  async function addAccount() {
    saving = true;
    error = null;
    try {
      const added = await addTotpAccount({
        label,
        issuer: issuer.trim() || null,
        account_name: accountName.trim() || null,
        secret_or_uri: secretOrUri,
        backup_codes: backupCodes.trim() || null
      });
      label = '';
      issuer = '';
      accountName = '';
      secretOrUri = '';
      backupCodes = '';
      selectedId = added.id;
      codeReport = null;
      await refresh();
      onToast('Authenticator account added');
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (handleSessionError(msg)) return;
      error = msg;
    } finally {
      saving = false;
    }
  }

  async function showCode(id: string) {
    error = null;
    selectedId = id;
    try {
      codeReport = await getTotpCode(id);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (handleSessionError(msg)) return;
      error = msg;
    }
  }

  function selectedAccount(): TotpAccountDto | null {
    return accounts.find((account) => account.id === selectedId) ?? null;
  }

  function fmtDate(s: string | null): string {
    if (!s) return '—';
    try {
      return new Date(s).toLocaleString();
    } catch {
      return s;
    }
  }

  $effect(() => {
    refresh();
  });
</script>

<div class="modal-backdrop" role="presentation">
  <div class="modal wide-modal" role="dialog" aria-modal="true" aria-labelledby="auth-title">
    <div class="modal-head">
      <div>
        <p class="eyebrow">Holster Auth</p>
        <h2 id="auth-title">Authenticator codes</h2>
        <p class="muted">TOTP secrets and backup codes stay encrypted in the local vault.</p>
      </div>
      <button class="ghost icon-button" onclick={onClose} aria-label="Close">×</button>
    </div>

    {#if error}
      <div class="error-box">{error}</div>
    {/if}

    <div class="auth-grid">
      <section class="auth-panel">
        <div class="section-heading tight-heading">
          <div>
            <h3>Accounts</h3>
            <p>{accounts.length} authenticator account{accounts.length === 1 ? '' : 's'}</p>
          </div>
        </div>

        {#if loading && accounts.length === 0}
          <div class="empty compact-empty">Loading…</div>
        {:else if accounts.length === 0}
          <div class="empty compact-empty">
            <p class="empty-title">No 2FA accounts yet.</p>
            <p>Add the secret key or paste the otpauth URI from a QR code setup screen.</p>
          </div>
        {:else}
          <div class="auth-account-list">
            {#each accounts as account (account.id)}
              <button
                class:selected-auth={selectedId === account.id}
                class="auth-account"
                onclick={() => showCode(account.id)}
              >
                <span class="auth-label">{account.label}</span>
                <span class="auth-meta">{account.issuer || 'Authenticator'}{account.account_name ? ` · ${account.account_name}` : ''}</span>
                <span class="auth-meta">{account.backup_code_count} backup code{account.backup_code_count === 1 ? '' : 's'} saved</span>
              </button>
            {/each}
          </div>
        {/if}

        {#if selectedAccount()}
          <div class="code-card">
            <span class="summary-label">Current code</span>
            {#if codeReport}
              <strong class="totp-code">{codeReport.code}</strong>
              <span class="summary-copy">Expires in {codeReport.seconds_remaining}s</span>
            {:else}
              <button class="primary" onclick={() => selectedId && showCode(selectedId)}>Show code</button>
            {/if}
            <p class="muted">Last used: {fmtDate(selectedAccount()?.last_used_at ?? null)}</p>
          </div>
        {/if}
      </section>

      <section class="auth-panel">
        <div class="section-heading tight-heading">
          <div>
            <h3>Add account</h3>
            <p>Manual secret today. QR image scan comes next.</p>
          </div>
        </div>

        <div class="field">
          <label for="totp-label">Label</label>
          <input id="totp-label" bind:value={label} placeholder="Cloudflare" />
        </div>
        <div class="two-col">
          <div class="field">
            <label for="totp-issuer">Issuer</label>
            <input id="totp-issuer" bind:value={issuer} placeholder="Cloudflare" />
          </div>
          <div class="field">
            <label for="totp-account">Account</label>
            <input id="totp-account" bind:value={accountName} placeholder="dave@example.com" />
          </div>
        </div>
        <div class="field">
          <label for="totp-secret">Secret or otpauth URI</label>
          <textarea
            id="totp-secret"
            rows="4"
            bind:value={secretOrUri}
            placeholder="JBSWY3DPEHPK3PXP or otpauth://totp/..."
          ></textarea>
          <p class="muted">The raw secret is encrypted immediately and never shown again.</p>
        </div>
        <div class="field">
          <label for="totp-backup">Backup codes</label>
          <textarea
            id="totp-backup"
            rows="5"
            bind:value={backupCodes}
            placeholder="One recovery code per line"
          ></textarea>
        </div>
        <div class="modal-actions">
          <button class="ghost" onclick={onClose}>Close</button>
          <button class="primary" onclick={addAccount} disabled={saving || !label.trim() || !secretOrUri.trim()}>
            {saving ? 'Saving…' : 'Add authenticator'}
          </button>
        </div>
      </section>
    </div>
  </div>
</div>
