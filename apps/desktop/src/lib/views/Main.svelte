<script lang="ts">
  import { listKeys, lockVault, copyToClipboard, type KeyMetadataDto } from '$lib/api';
  import AddKeyDialog from './AddKeyDialog.svelte';
  import ConfirmDelete from './ConfirmDelete.svelte';
  import ExportRuntimeDialog from './ExportRuntimeDialog.svelte';
  import ScanProjectDialog from './ScanProjectDialog.svelte';
  import HolsterDoctorDialog from './HolsterDoctorDialog.svelte';
  import BuildbeltSetupDialog from './BuildbeltSetupDialog.svelte';
  import GitignoreHelperDialog from './GitignoreHelperDialog.svelte';
  import EnvExampleDialog from './EnvExampleDialog.svelte';
  import AuthDialog from './AuthDialog.svelte';

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
  let showExport = $state(false);
  let showScan = $state(false);
  let showDoctor = $state(false);
  let showBuildbelt = $state(false);
  let buildbeltStartupMode = $state(false);
  let startupComplete = $state(false);
  let startupAudience = $state<'personal' | 'business'>('personal');
  let showAdvancedTools = $state(false);
  let showGitignore = $state(false);
  let showEnvExample = $state(false);
  let showAuth = $state(false);
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

  function openBuildbelt(startup = false) {
    buildbeltStartupMode = startup;
    showBuildbelt = true;
  }

  function refreshStartupState() {
    try {
      startupComplete = localStorage.getItem('buildbeltStartupComplete') === 'true';
      const progress = JSON.parse(localStorage.getItem('buildbeltStartupProgress') ?? '{}');
      startupAudience = progress.startupAudience === 'business' ? 'business' : 'personal';
    } catch {
      startupComplete = false;
      startupAudience = 'personal';
    }
    showAdvancedTools = false;
  }

  function resetStartup() {
    try {
      localStorage.removeItem('buildbeltStartupComplete');
      localStorage.removeItem('buildbeltStartupProgress');
    } catch {
      // Startup can still restart in this session.
    }
    startupComplete = false;
    startupAudience = 'personal';
    showAdvancedTools = false;
    openBuildbelt(true);
  }

  function fmtDate(s: string | null): string {
    if (!s) return '—';
    try {
      return new Date(s).toLocaleString();
    } catch {
      return s;
    }
  }

  function providerCount(): number {
    return new Set(keys.map((k) => k.provider)).size;
  }

  function recentlyUsedCount(): number {
    const weekAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
    return keys.filter((k) => k.last_used_at && new Date(k.last_used_at).getTime() >= weekAgo).length;
  }

  // Poll list_keys every 60s — this serves as both a "refresh metadata" tick
  // (last_used_at can change) and a session-expiry probe. If the vault has
  // auto-locked due to idle, the next list_keys returns SessionExpired and
  // we'll bounce back to the unlock screen.
  let pollTimer: ReturnType<typeof setInterval> | undefined;
  $effect(() => {
    refresh();
    pollTimer = setInterval(refresh, 60_000);
    refreshStartupState();
    try {
      if (localStorage.getItem('buildbeltStartupComplete') !== 'true') {
        openBuildbelt(true);
      }
    } catch {
      openBuildbelt(true);
    }
    return () => {
      if (pollTimer) clearInterval(pollTimer);
    };
  });
</script>

<div class="app-shell" class:level-shell={startupComplete && !showAdvancedTools}>
  <aside class="side-rail" aria-label="Holster modules">
    <div class="brand-block">
      <div class="brand-mark">N</div>
      <div>
        <div class="brand">Buildbelt</div>
        <div class="brand-subtitle">by NautaAI</div>
      </div>
    </div>

    <nav class="module-nav" aria-label="Product modules">
      <button class="module-item nav-button buildbelt-nav" onclick={() => openBuildbelt(false)}>
        <span class="module-dot"></span>
        <span>Setup</span>
        <span class="soon">alpha</span>
      </button>
      <button class="module-item nav-button doctor-nav" onclick={() => (showDoctor = true)}>
        <span class="module-dot"></span>
        <span>Doctor</span>
        <span class="soon">v0</span>
      </button>
      <div class="module-item active">
        <span class="module-dot"></span>
        <span>Secrets</span>
      </div>
      <button class="module-item nav-button" onclick={() => (showAuth = true)}>
        <span class="module-dot"></span>
        <span>Auth</span>
        <span class="soon">new</span>
      </button>
      <div class="module-item muted-module">
        <span class="module-dot idle"></span>
        <span>Models</span>
        <span class="soon">next</span>
      </div>
      <div class="module-item muted-module">
        <span class="module-dot idle"></span>
        <span>Sessions</span>
        <span class="soon">planned</span>
      </div>
      <div class="module-item muted-module">
        <span class="module-dot idle"></span>
        <span>Launch</span>
        <span class="soon">planned</span>
      </div>
    </nav>

    <div class="rail-note">
      Local-first. No cloud sync. Secret values stay behind the vault boundary.
    </div>
  </aside>

  <section class="workspace">
    <header class="topbar">
      <div>
        <p class="eyebrow">Doctor</p>
        <h1>Turn AI confusion into a safe local setup path.</h1>
      </div>
      <div class="actions">
        <button onclick={() => openBuildbelt(false)} class="primary">Start AI Setup</button>
        <button onclick={() => (showDoctor = true)} class="primary">Run Doctor</button>
        <button onclick={() => (showAdd = true)} class="primary">Add key</button>
        <button onclick={refresh} class="ghost icon-button" title="Refresh" aria-label="Refresh">↻</button>
        <button onclick={onLock} class="ghost">Lock</button>
      </div>
    </header>

    <main>
    {#if error}
      <div class="error-box">{error}</div>
    {/if}

    {#if startupComplete && !showAdvancedTools}
      <section class="level-landing" aria-label="Buildbelt next level">
        <div class="level-brand-mark">N</div>
        <p class="eyebrow">Buildbelt by NautaAI</p>
        {#if startupAudience === 'business'}
          <h2>Level 1 complete.</h2>
          <p>You have the safe starting line. Next, decide how your team moves from simple AI use into a real business system.</p>
          <div class="level-next">
            <span>Next level</span>
            <strong>Build the team system.</strong>
            <p>Choose approved tools, owner controls, file rules, and when a workflow is ready for keys or agents.</p>
          </div>
          <div class="level-actions">
            <button class="primary" onclick={() => openBuildbelt(false)}>Start Level 2</button>
            <div class="level-secondary-actions">
              <button class="text-button" onclick={resetStartup}>Restart Level 1</button>
              <button class="text-button" onclick={() => (showAdvancedTools = true)}>Advanced tools</button>
            </div>
          </div>
        {:else}
          <h2>Level 1 complete.</h2>
          <p>You have started with one AI app. Next, decide whether to keep using what you have or build a real AI system.</p>
          <div class="level-next">
            <span>Next level</span>
            <strong>Build your system.</strong>
            <p>Choose old computer or new system, secure the account, and add tools only when there is a real workflow.</p>
          </div>
          <div class="level-actions">
            <button class="primary" onclick={() => openBuildbelt(false)}>Start Level 2</button>
            <div class="level-secondary-actions">
              <button class="text-button" onclick={resetStartup}>Restart Level 1</button>
              <button class="text-button" onclick={() => (showAdvancedTools = true)}>Advanced tools</button>
            </div>
          </div>
        {/if}
      </section>
    {:else}
    <section class="buildbelt-banner" aria-label="Buildbelt setup">
      <div>
        <p class="eyebrow">Buildbelt Alpha</p>
        <h2>Start before API billing, expensive hardware, or unsafe agent handoffs.</h2>
        <p>
          Buildbelt walks beginners from one predictable AI subscription to
          account safety, key storage, safe sharing, and workstation readiness.
        </p>
      </div>
      <button class="primary" onclick={() => openBuildbelt(false)}>Open setup guide</button>
    </section>

    <section class="doctor-banner" aria-label="Holster Doctor">
      <div>
        <p class="eyebrow">Holster Doctor V0</p>
        <h2>A local safety report before an agent touches your repo.</h2>
        <p>
          Doctor turns secret detection, .gitignore hygiene, runtime template
          readiness, and agent profiles into one buyer-grade handoff report.
        </p>
      </div>
      <button class="primary" onclick={() => (showDoctor = true)}>Generate report</button>
    </section>

    <section class="summary-grid" aria-label="Vault summary">
      <div class="summary-panel accent-panel">
        <span class="summary-label">Vault status</span>
        <strong>Unlocked</strong>
        <span class="summary-copy">Ready for local-only scans and runtime exports.</span>
      </div>
      <div class="summary-panel">
        <span class="summary-label">Stored keys</span>
        <strong>{keys.length}</strong>
        <span class="summary-copy">{providerCount()} provider{providerCount() === 1 ? '' : 's'} available for safe templates</span>
      </div>
      <div class="summary-panel">
        <span class="summary-label">Recently used</span>
        <strong>{recentlyUsedCount()}</strong>
        <span class="summary-copy">Used in the last 7 days</span>
      </div>
    </section>

    <section class="tool-strip" aria-label="Secret safety tools">
      <button class="tool-primary" onclick={() => (showDoctor = true)}>Holster Doctor</button>
      <button onclick={() => (showExport = true)} disabled={keys.length === 0}>
        Export runtime
      </button>
      <button onclick={() => (showScan = true)}>Scan project</button>
      <button onclick={() => (showGitignore = true)} title="Audit project .gitignore">
        Review .gitignore
      </button>
      <button onclick={() => (showEnvExample = true)} title="Generate a committable .env.example template">
        Generate .env.example
      </button>
      <button onclick={() => (showAuth = true)} title="Store 2FA authenticator codes in the vault">
        Holster Auth
      </button>
    </section>

    {#if loading && keys.length === 0}
      <div class="empty">Loading…</div>
    {:else if keys.length === 0}
      <div class="empty">
        <p class="empty-title">No keys yet.</p>
        <p>Run Doctor on a project first, then add provider keys when you are ready to generate runtime templates.</p>
        <div class="empty-actions">
          <button class="primary" onclick={() => (showDoctor = true)}>Run Doctor</button>
          <button onclick={() => (showAdd = true)}>Add your first key</button>
        </div>
      </div>
    {:else}
      <section class="table-panel">
        <div class="section-heading">
          <div>
            <h2>Vault inventory</h2>
            <p>Copy values only when needed. Runtime exports never expose values to the UI.</p>
          </div>
        </div>
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
                <td class="label-cell">{k.label}</td>
                <td>{k.project_tag ?? '—'}</td>
                <td class="id-cell">{fmtDate(k.created_at)}</td>
                <td class="id-cell">{fmtDate(k.last_used_at)}</td>
                <td>
                  <div class="row-actions">
                    <button onclick={() => onCopy(k)}>Copy</button>
                    <button class="danger ghost-danger" onclick={() => (confirmTarget = k)}>Delete</button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}
    {/if}
    </main>
  </section>
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

{#if showExport}
  <ExportRuntimeDialog
    {keys}
    onClose={() => (showExport = false)}
    onSessionExpired={onSessionExpired}
    onExported={(message) => showToast(message)}
  />
{/if}

{#if showScan}
  <ScanProjectDialog onClose={() => (showScan = false)} />
{/if}

{#if showDoctor}
  <HolsterDoctorDialog
    {keys}
    onClose={() => (showDoctor = false)}
    onSessionExpired={onSessionExpired}
  />
{/if}

{#if showBuildbelt}
  <BuildbeltSetupDialog
    startupMode={buildbeltStartupMode}
    onClose={() => {
      showBuildbelt = false;
      buildbeltStartupMode = false;
      refreshStartupState();
    }}
    onStartupDone={() => {
      showBuildbelt = false;
      buildbeltStartupMode = false;
      refreshStartupState();
    }}
    onOpenDoctor={() => (showDoctor = true)}
  />
{/if}

{#if showGitignore}
  <GitignoreHelperDialog onClose={() => (showGitignore = false)} />
{/if}

{#if showEnvExample}
  <EnvExampleDialog
    {keys}
    onClose={() => (showEnvExample = false)}
    onSessionExpired={onSessionExpired}
    onApplied={(message) => showToast(message)}
  />
{/if}

{#if showAuth}
  <AuthDialog
    onClose={() => (showAuth = false)}
    onSessionExpired={onSessionExpired}
    onToast={(message) => showToast(message)}
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
