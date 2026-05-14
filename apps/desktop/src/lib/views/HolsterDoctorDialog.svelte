<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    scanProject,
    gitignoreAudit,
    envExampleFromVault,
    listAgentProfiles,
    isFixtureClassification,
    type AgentProfile,
    type EnvExampleProposal,
    type GitignoreAuditReport,
    type KeyMetadataDto,
    type ScanDetection,
    type ScanReport,
    type ScanRiskLevel
  } from '$lib/api';
  import { recordScan } from '$lib/scanHistory';

  interface Props {
    keys: KeyMetadataDto[];
    onClose: () => void;
    onSessionExpired: () => void;
    initialPath?: string;
  }
  let { keys, onClose, onSessionExpired, initialPath = '' }: Props = $props();

  let projectPath = $state('');

  // Initialize the path field from the prop when the dialog opens (or if the
  // parent re-mounts with a different starting path).
  $effect(() => {
    if (initialPath) {
      projectPath = initialPath;
    }
  });
  let busy = $state(false);
  let error = $state<string | null>(null);
  let scanReport = $state<ScanReport | null>(null);
  let gitignoreReport = $state<GitignoreAuditReport | null>(null);
  let envProposal = $state<EnvExampleProposal | null>(null);
  let profiles = $state<AgentProfile[]>([]);

  // Risk counts the verdict + headline use — fixture-classified detections
  // are kept in `summary_by_risk` (raw) but excluded here so the report
  // doesn't flag a healthy repo with intentional test data as critical.
  const riskCounts = $derived.by(() => scanReport?.summary_by_risk_excluding_fixtures ?? {});
  const realFindingCount = $derived.by(() => scanReport?.real_finding_count ?? 0);
  const fixtureFindingCount = $derived.by(() => scanReport?.fixture_finding_count ?? 0);
  const realDetections = $derived.by<ScanDetection[]>(() =>
    (scanReport?.detections ?? []).filter((d) => !isFixtureClassification(d.classification))
  );
  const fixtureDetections = $derived.by<ScanDetection[]>(() =>
    (scanReport?.detections ?? []).filter((d) => isFixtureClassification(d.classification))
  );
  const missingGitignoreLines = $derived.by(() => {
    if (!gitignoreReport) return 0;
    return gitignoreReport.rule_sets.reduce((total, set) => {
      return total + set.rules.filter((rule) => !rule.already_present).length;
    }, 0);
  });
  const verdict = $derived.by(() => {
    const critical = riskCounts.critical ?? 0;
    const high = riskCounts.high ?? 0;
    const medium = riskCounts.medium ?? 0;
    if (!scanReport) return { label: 'Ready to scan', tone: 'neutral', copy: 'Generate a local safety report before you share, paste, or let an agent modify this repo.' };
    if (critical > 0 || high > 0) {
      return { label: 'Not handoff-ready', tone: 'danger', copy: 'High-risk findings need cleanup before this project is safe for agent access.' };
    }
    if (medium > 0 || missingGitignoreLines > 0) {
      return { label: 'Review advised', tone: 'warn', copy: 'No critical exposure found. Finish the hygiene checklist before sharing the project.' };
    }
    return { label: 'Handoff-ready', tone: 'safe', copy: 'No exposed secrets found. The repo is ready for a controlled agent handoff.' };
  });

  function sessionExpired(msg: string) {
    return msg.toLowerCase().includes('session expired') || msg.toLowerCase().includes('session is invalid');
  }

  async function chooseFolder() {
    error = null;
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Choose a project folder for Holster Doctor'
      });
      if (typeof selected === 'string') {
        projectPath = selected;
        clearReports();
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function clearReports() {
    scanReport = null;
    gitignoreReport = null;
    envProposal = null;
    profiles = [];
  }

  async function runDoctor(e?: SubmitEvent) {
    e?.preventDefault();
    error = null;
    clearReports();
    if (!projectPath.trim()) {
      error = 'Choose a project folder first.';
      return;
    }
    busy = true;
    try {
      scanReport = await scanProject({
        path: projectPath.trim(),
        follow_symlinks: false,
        respect_gitignore: false,
        max_file_size_bytes: 5_000_000
      });
      // Record metadata-only history entry so the main view can list recent scans.
      // recordScan stores no raw findings or values.
      if (scanReport) {
        recordScan(scanReport);
      }
      gitignoreReport = await gitignoreAudit({ path: projectPath.trim() });
      profiles = await listAgentProfiles();
      if (keys.length > 0) {
        envProposal = await envExampleFromVault({
          key_ids: keys.map((key) => key.id),
          include_holster_comments: false
        });
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (sessionExpired(msg)) {
        onSessionExpired();
        return;
      }
      error = msg;
    } finally {
      busy = false;
    }
  }

  function fmtElapsed(ms: number): string {
    if (ms < 1000) return `${ms} ms`;
    return `${(ms / 1000).toFixed(2)} s`;
  }

  function riskClass(r: ScanRiskLevel): string {
    return `risk-${r}`;
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
  <div class="modal doctor-modal" role="dialog" aria-modal="true" aria-labelledby="doctor-title">
    <div class="doctor-hero">
      <div>
        <p class="eyebrow">Holster Doctor</p>
        <h2 id="doctor-title">Generate a handoff safety report.</h2>
        <p>
          Doctor checks the repo, the handoff boundary, and the runtime template
          before an AI agent gets access. Secret values stay local and redacted.
        </p>
      </div>
      <div class="doctor-verdict {verdict.tone}">
        <span>{verdict.label}</span>
        <strong>{scanReport ? realFindingCount : '—'}</strong>
        <small>findings</small>
        {#if scanReport && fixtureFindingCount > 0}
          <small class="fixture-note">+ {fixtureFindingCount} test fixture{fixtureFindingCount === 1 ? '' : 's'}</small>
        {/if}
      </div>
    </div>

    <form class="doctor-picker" onsubmit={runDoctor}>
      <div class="field">
        <label for="doctor-path">Project folder</label>
        <div class="path-picker">
          <input
            id="doctor-path"
            type="text"
            bind:value={projectPath}
            placeholder="/Users/admin/my-agent-project"
            disabled={busy}
          />
          <button type="button" onclick={chooseFolder} disabled={busy}>Browse</button>
        </div>
      </div>
      <div class="doctor-actions">
        <button type="button" class="ghost" onclick={onClose} disabled={busy}>Close</button>
        <button type="submit" class="primary" disabled={busy}>
          {busy ? 'Building report…' : scanReport ? 'Refresh report' : 'Generate safety report'}
        </button>
      </div>
    </form>

    {#if error}
      <div class="error-box">{error}</div>
    {/if}

    <section class="doctor-summary" aria-label="Holster Doctor result">
      <article class="doctor-card verdict-card {verdict.tone}">
        <span>Verdict</span>
        <strong>{verdict.label}</strong>
        <p>{verdict.copy}</p>
      </article>
      <article class="doctor-card">
        <span>Secrets</span>
        <strong>{scanReport ? realFindingCount : '—'}</strong>
        <p>
          {#if scanReport}
            {scanReport.scanned_files} files scanned in {fmtElapsed(scanReport.elapsed_ms)}{fixtureFindingCount > 0 ? ` · ${fixtureFindingCount} test fixture${fixtureFindingCount === 1 ? '' : 's'} excluded from verdict` : ''}
          {:else}
            Redacted detector report
          {/if}
        </p>
      </article>
      <article class="doctor-card">
        <span>.gitignore</span>
        <strong>{gitignoreReport ? missingGitignoreLines : '—'}</strong>
        <p>{gitignoreReport ? 'missing recommended lines' : 'append-only safety audit'}</p>
      </article>
      <article class="doctor-card">
        <span>Runtime</span>
        <strong>{envProposal ? envProposal.lines.length : keys.length}</strong>
        <p>{envProposal ? '.env.example placeholders available' : 'vault keys available'}</p>
      </article>
    </section>

    {#if scanReport}
      <section class="doctor-report-grid">
        <article class="doctor-panel">
          <div class="panel-head">
            <h3>Risk Breakdown</h3>
            <span>{realFindingCount} real{fixtureFindingCount > 0 ? ` · ${fixtureFindingCount} fixture${fixtureFindingCount === 1 ? '' : 's'}` : ''}</span>
          </div>
          <div class="risk-ladder">
            {#each ['critical', 'high', 'medium', 'low'] as risk}
              <div>
                <span class="chip {riskClass(risk as ScanRiskLevel)}">{risk}</span>
                <strong>{riskCounts[risk] ?? 0}</strong>
              </div>
            {/each}
          </div>
        </article>

        <article class="doctor-panel">
          <div class="panel-head">
            <h3>Safe Next Actions</h3>
            <span>local only</span>
          </div>
          <ol class="doctor-next">
            {#if realFindingCount > 0}
              <li>Rotate or remove critical/high findings before granting agent access.</li>
            {:else}
              <li>Secret detector pass is clean for this folder.</li>
            {/if}
            {#if missingGitignoreLines > 0}
              <li>Add the missing .gitignore protections before committing or sharing.</li>
            {:else}
              <li>.gitignore protections are already covered.</li>
            {/if}
            <li>Export a committable .env.example and agent profile for the next tool.</li>
            {#if fixtureFindingCount > 0}
              <li>Review the test-fixture panel below if any fixture is actually a real key.</li>
            {/if}
          </ol>
        </article>
      </section>

      {#if realDetections.length > 0}
        <section class="doctor-panel">
          <div class="panel-head">
            <h3>Top Findings</h3>
            <span>no raw values</span>
          </div>
          <div class="doctor-findings">
            {#each realDetections.slice(0, 6) as finding}
              <div class="doctor-finding">
                <div>
                  <span class="chip {riskClass(finding.risk_level)}">{finding.risk_level}</span>
                  <strong>{finding.display_name}</strong>
                  <span class="provider-badge">{finding.provider}</span>
                </div>
                <code>{finding.file_path ?? '(unknown path)'}:{finding.line_number}</code>
                <p>{finding.recommended_action}</p>
              </div>
            {/each}
          </div>
        </section>
      {/if}

      {#if fixtureDetections.length > 0}
        <section class="doctor-panel doctor-panel-fixtures">
          <div class="panel-head">
            <h3>Test fixtures (informational)</h3>
            <span>{fixtureDetections.length} excluded from verdict</span>
          </div>
          <p class="muted">
            These match the secret patterns but live in test paths or use known
            placeholder shapes (e.g. <code>sk-test-</code>, <code>FAKE</code>,
            long zero runs). They don't drive the verdict — but if any one is
            actually a real key that ended up in test code, treat it as a real
            finding.
          </p>
          <div class="doctor-findings">
            {#each fixtureDetections.slice(0, 6) as finding}
              <div class="doctor-finding doctor-finding-fixture">
                <div>
                  <span class="chip risk-low">fixture</span>
                  <strong>{finding.display_name}</strong>
                  <span class="provider-badge">{finding.provider}</span>
                </div>
                <code>{finding.file_path ?? '(unknown path)'}:{finding.line_number}</code>
                <p>{finding.recommended_action}</p>
              </div>
            {/each}
          </div>
        </section>
      {/if}

      <section class="doctor-report-grid">
        <article class="doctor-panel">
          <div class="panel-head">
            <h3>Agent Profiles</h3>
            <span>{profiles.length}</span>
          </div>
          <div class="profile-list">
            {#each profiles.slice(0, 4) as profile}
              <div>
                <strong>{profile.name}</strong>
                <span>{profile.default_filename}</span>
              </div>
            {/each}
          </div>
        </article>

        <article class="doctor-panel">
          <div class="panel-head">
            <h3>.env.example Preview</h3>
            <span>{envProposal ? `${envProposal.lines.length} vars` : 'not ready'}</span>
          </div>
          {#if envProposal}
            <pre>{envProposal.lines.slice(0, 8).map((line) => `${line.name}=`).join('\n')}</pre>
          {:else}
            <p class="muted">Add vault keys to generate placeholder-only runtime templates without exposing values.</p>
          {/if}
        </article>
      </section>
    {/if}
  </div>
</div>
