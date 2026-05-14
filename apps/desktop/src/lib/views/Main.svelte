<script lang="ts">
  import { listKeys, lockVault, type KeyMetadataDto } from '$lib/api';
  import { loadScanHistory, type ScanHistoryEntry } from '$lib/scanHistory';
  import AddKeyDialog from './AddKeyDialog.svelte';
  import ExportRuntimeDialog from './ExportRuntimeDialog.svelte';
  import HolsterDoctorDialog from './HolsterDoctorDialog.svelte';
  import GitignoreHelperDialog from './GitignoreHelperDialog.svelte';
  import EnvExampleDialog from './EnvExampleDialog.svelte';
  import AuthDialog from './AuthDialog.svelte';
  import McpPreflightDialog from './McpPreflightDialog.svelte';
  import SecureKeysDialog from './SecureKeysDialog.svelte';
  import ProjectToolsDialog from './ProjectToolsDialog.svelte';

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
  let showSecureKeys = $state(false);
  let showProjectTools = $state(false);
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

  function verdictTone(entry: ScanHistoryEntry): 'risk-safe' | 'risk-warn' | 'risk-danger' {
    const counts = entry.summary_by_risk_excluding_fixtures ?? {};
    const critical = counts.critical ?? 0;
    const high = counts.high ?? 0;
    if (critical > 0 || high > 0) return 'risk-danger';
    const medium = counts.medium ?? 0;
    if (medium > 0) return 'risk-warn';
    return 'risk-safe';
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
      <button class="module-item nav-button" onclick={() => (showProjectTools = true)}>
        <span class="module-dot"></span>
        <span>Project Tools</span>
      </button>
      <button class="module-item nav-button" onclick={() => (showSecureKeys = true)}>
        <span class="module-dot"></span>
        <span>Vault</span>
      </button>
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

{#if showSecureKeys}
  <SecureKeysDialog
    onClose={() => {
      showSecureKeys = false;
      refresh();
    }}
    onSessionExpired={onSessionExpired}
    onToast={(message) => showToast(message)}
  />
{/if}

{#if showProjectTools}
  <ProjectToolsDialog
    exportDisabled={keys.length === 0}
    onClose={() => (showProjectTools = false)}
    onEnvExample={() => {
      showProjectTools = false;
      showEnvExample = true;
    }}
    onGitignore={() => {
      showProjectTools = false;
      showGitignore = true;
    }}
    onExportProfile={() => {
      showProjectTools = false;
      showExport = true;
    }}
  />
{/if}

{#if toast}
  <div class="toast">{toast}</div>
{/if}
