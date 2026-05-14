<script lang="ts">
  import { listKeys, lockVault, copyToClipboard, type KeyMetadataDto } from '$lib/api';
  import { loadScanHistory, type ScanHistoryEntry } from '$lib/scanHistory';
  import AddKeyDialog from './AddKeyDialog.svelte';
  import ConfirmDelete from './ConfirmDelete.svelte';
  import ExportRuntimeDialog from './ExportRuntimeDialog.svelte';
  import HolsterDoctorDialog from './HolsterDoctorDialog.svelte';
  import GitignoreHelperDialog from './GitignoreHelperDialog.svelte';
  import EnvExampleDialog from './EnvExampleDialog.svelte';
  import AuthDialog from './AuthDialog.svelte';
  import McpPreflightDialog from './McpPreflightDialog.svelte';

  interface Props {
    onLocked: () => void;
    onSessionExpired: () => void;
  }
  let { onLocked, onSessionExpired }: Props = $props();

  let keys = $state<KeyMetadataDto[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let toast = $state<string | null>(null);
  let scanHistory = $state<ScanHistoryEntry[]>([]);
  let showAdd = $state(false);
  let showExport = $state(false);
  let showDoctor = $state(false);
  let showGitignore = $state(false);
  let showEnvExample = $state(false);
  let showAuth = $state(false);
  let showMcp = $state(false);
  let confirmTarget = $state<KeyMetadataDto | null>(null);
  let initialDoctorPath = $state('');

  async function refresh() {
    loading = true;
    error = null;
    try {
      keys = await listKeys();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (msg.toLowerCase().includes('session expired') || msg.toLowerCase().includes('session is invalid')) {
        onSessionExpired();
        return;
      }
      error = msg;
    } finally {
      loading = false;
    }
  }

  function refreshScanHistory() {
    scanHistory = loadScanHistory();
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

  function openDoctor(path = '') {
    initialDoctorPath = path;
    showDoctor = true;
  }

  function fmtDate(s: string | null): string {
    if (!s) return '—';
    try {
      return new Date(s).toLocaleString();
    } catch {
      return s;
    }
  }

  function fmtRelativeTime(timestamp: number): string {
    const diff = Date.now() - timestamp;
    const min = Math.floor(diff / 60_000);
    const hr = Math.floor(diff / 3_600_000);
    const day = Math.floor(diff / 86_400_000);
    if (day > 0) return `${day}d ago`;
    if (hr > 0) return `${hr}h ago`;
    if (min > 0) return `${min}m ago`;
    return 'just now';
  }

  function shortenPath(path: string, maxLen = 60): string {
    if (path.length <= maxLen) return path;
    const head = path.slice(0, 12);
    const tail = path.slice(path.length - (maxLen - 15));
    return `${head}…${tail}`;
  }

  function verdictTone(entry: ScanHistoryEntry): 'safe' | 'warn' | 'danger' {
    const counts = entry.summary_by_risk_excluding_fixtures ?? {};
    const critical = counts.critical ?? 0;
    const high = counts.high ?? 0;
    if (critical > 0 || high > 0) return 'danger';
    const medium = counts.medium ?? 0;
    if (medium > 0) return 'warn';
    return 'safe';
  }

  function verdictLabel(entry: ScanHistoryEntry): string {
    const tone = verdictTone(entry);
    if (tone === 'danger') return 'Not handoff-ready';
    if (tone === 'warn') return 'Review advised';
    return 'Handoff-ready';
  }

  let pollTimer: ReturnType<typeof setInterval> | undefined;
  $effect(() => {
    refresh();
    refreshScanHistory();
    pollTimer = setInterval(refresh, 60_000);
    return () => {
      if (pollTimer) clearInterval(pollTimer);
    };
  });
</script>

<div class="app-shell">
  <aside class="side-rail" aria-label="Holster navigation">
    <div class="brand-block">
      <img class="brand-mark-img" src="/holster-mark.png" alt="Holster" />
      <div>
        <div class="brand">Holster</div>
        <div class="brand-subtitle">by Nauta AI</div>
      </div>
    </div>

    <nav class="module-nav" aria-label="Modules">
      <div class="module-item active">
        <span class="module-dot"></span>
        <span>Doctor</span>
      </div>
      <button class="module-item nav-button" onclick={() => (showAuth = true)}>
        <span class="module-dot"></span>
        <span>Auth</span>
      </button>
      <button class="module-item nav-button" onclick={() => (showMcp = true)}>
        <span class="module-dot"></span>
        <span>MCP Preflight</span>
      </button>
    </nav>

    <div class="rail-note">
      Local-first. No cloud sync. Secret values stay behind the vault boundary.
    </div>
  </aside>

  <section class="workspace">
    <header class="topbar">
      <div>
        <p class="eyebrow">Holster</p>
        <h1>Local preflight before AI agents touch your repo.</h1>
      </div>
      <div class="actions">
        <button onclick={() => openDoctor()} class="primary">Scan a project</button>
        <button onclick={refresh} class="ghost icon-button" title="Refresh" aria-label="Refresh">↻</button>
        <button onclick={onLock} class="ghost">Lock</button>
      </div>
    </header>

    <main>
      {#if error}
        <div class="error-box">{error}</div>
      {/if}

      {#if scanHistory.length === 0}
        <section class="hero-empty" aria-label="Get started">
          <p class="eyebrow">Start here</p>
          <h2>Run your first scan.</h2>
          <p>
            Pick any project folder. Holster checks what is safe to run, what is safe to share,
            and what to fix first — with no values leaving your machine.
          </p>
          <button class="primary" onclick={() => openDoctor()}>Choose a folder</button>
        </section>
      {:else}
        <section class="recent-scans table-panel" aria-label="Recent scans">
          <div class="section-heading">
            <div>
              <h2>Recent scans</h2>
              <p>Click a row to re-scan that path.</p>
            </div>
            <button class="primary" onclick={() => openDoctor()}>Scan a project</button>
          </div>
          <ul class="scan-list">
            {#each scanHistory.slice(0, 5) as entry (entry.root_path)}
              <li>
                <button class="scan-row {verdictTone(entry)}" onclick={() => openDoctor(entry.root_path)} title={entry.root_path}>
                  <span class="scan-verdict">{verdictLabel(entry)}</span>
                  <span class="scan-path">{shortenPath(entry.root_path)}</span>
                  <span class="scan-meta">
                    {entry.real_finding_count} finding{entry.real_finding_count === 1 ? '' : 's'}
                    {#if entry.fixture_finding_count > 0}
                      · {entry.fixture_finding_count} fixture{entry.fixture_finding_count === 1 ? '' : 's'}
                    {/if}
                    · {fmtRelativeTime(entry.timestamp)}
                  </span>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <section class="tool-strip" aria-label="Tools">
        <button onclick={() => (showEnvExample = true)} title="Generate a committable .env.example template">
          Generate .env.example
        </button>
        <button onclick={() => (showGitignore = true)} title="Audit project .gitignore">
          Review .gitignore
        </button>
        <button onclick={() => (showExport = true)} disabled={keys.length === 0} title="Export an agent runtime profile">
          Export agent profile
        </button>
        <button onclick={() => (showAuth = true)} title="Store 2FA codes in the vault">
          Auth
        </button>
      </section>

      {#if loading && keys.length === 0}
        <div class="empty">Loading vault…</div>
      {:else if keys.length === 0}
        <section class="vault-empty empty">
          <p class="empty-title">Vault is empty.</p>
          <p>Add the API keys you use so Holster can generate matching <code>.env.example</code> placeholders and agent runtime profiles. Optional — scans work without it.</p>
          <div class="empty-actions">
            <button onclick={() => (showAdd = true)}>Add a key</button>
          </div>
        </section>
      {:else}
        <section class="table-panel">
          <div class="section-heading">
            <div>
              <h2>Vault</h2>
              <p>{keys.length} key{keys.length === 1 ? '' : 's'} stored locally. Values never appear in the UI.</p>
            </div>
            <button onclick={() => (showAdd = true)}>Add key</button>
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

{#if showDoctor}
  <HolsterDoctorDialog
    {keys}
    initialPath={initialDoctorPath}
    onClose={() => {
      showDoctor = false;
      refreshScanHistory();
    }}
    onSessionExpired={onSessionExpired}
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

{#if showMcp}
  <McpPreflightDialog onClose={() => (showMcp = false)} />
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
