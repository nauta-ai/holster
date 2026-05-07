<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    scanProject,
    type ScanReport,
    type ScanDetection,
    type ScanRiskLevel
  } from '$lib/api';

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  let path = $state('');
  let respectGitignore = $state(false);
  let followSymlinks = $state(false);
  let maxFileSizeMb = $state(5);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let report = $state<ScanReport | null>(null);

  // Findings filters
  let filterRisk = $state<ScanRiskLevel | 'all'>('all');
  let filterProvider = $state<string>('all');
  let filterGitTracked = $state<'all' | 'tracked' | 'untracked'>('all');

  const filteredDetections = $derived.by(() => {
    if (!report) return [];
    return report.detections.filter((d) => {
      if (filterRisk !== 'all' && d.risk_level !== filterRisk) return false;
      if (filterProvider !== 'all' && d.provider !== filterProvider) return false;
      if (filterGitTracked === 'tracked' && d.git_tracked !== true) return false;
      if (filterGitTracked === 'untracked' && d.git_tracked === true) return false;
      return true;
    });
  });

  const providersInReport = $derived.by(() => {
    if (!report) return [];
    return Object.keys(report.summary_by_provider).sort();
  });

  async function chooseFolder() {
    error = null;
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Choose a project folder to scan'
      });
      if (typeof selected === 'string') {
        path = selected;
        report = null;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function runScan(e?: SubmitEvent) {
    e?.preventDefault();
    error = null;
    report = null;
    if (!path.trim()) {
      error = 'Choose a project folder first.';
      return;
    }
    busy = true;
    try {
      report = await scanProject({
        path: path.trim(),
        follow_symlinks: followSymlinks,
        respect_gitignore: respectGitignore,
        max_file_size_bytes: Math.max(0, Math.floor(maxFileSizeMb * 1_000_000))
      });
      // Reset filters when a new report arrives
      filterRisk = 'all';
      filterProvider = 'all';
      filterGitTracked = 'all';
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  function riskClass(r: ScanRiskLevel): string {
    return `risk-${r}`;
  }

  function fmtElapsed(ms: number): string {
    if (ms < 1000) return `${ms} ms`;
    return `${(ms / 1000).toFixed(2)} s`;
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
  <div class="modal wide-modal" role="dialog" aria-modal="true" aria-labelledby="scan-title">
    <h2 id="scan-title">Scan project for leaked secrets</h2>
    <p class="muted" style="margin-bottom: 18px;">
      Walk a local project folder and check every text file for known API key formats.
      Findings are redacted previews only — the raw matched values never leave Rust.
    </p>
    <p class="muted" style="margin-bottom: 18px; font-size: 12px;">
      Skipped automatically: <code>.git</code>, <code>node_modules</code>, <code>target</code>,
      <code>dist</code>, <code>build</code>, <code>.next</code>, <code>vendor</code>,
      <code>.venv</code>, <code>__pycache__</code>, and similar build/cache dirs.
    </p>

    <form onsubmit={runScan}>
      <div class="field">
        <label for="scan-path">Project folder</label>
        <div class="path-picker">
          <input
            id="scan-path"
            type="text"
            bind:value={path}
            placeholder="/Users/admin/my-project"
            disabled={busy}
          />
          <button type="button" onclick={chooseFolder} disabled={busy}>Browse…</button>
        </div>
      </div>

      <div class="field options-row">
        <label class="check-row inline">
          <input type="checkbox" bind:checked={respectGitignore} />
          <span>
            Respect <code>.gitignore</code>
            <span class="muted" style="font-size: 11px;">
              (off by default — gitignored <code>.env</code> files are exactly what we want to find)
            </span>
          </span>
        </label>
        <label class="check-row inline">
          <input type="checkbox" bind:checked={followSymlinks} />
          <span>Follow symlinks</span>
        </label>
        <label class="check-row inline">
          <span>
            Max file size:
            <input
              type="number"
              min="1"
              max="100"
              step="1"
              bind:value={maxFileSizeMb}
              style="width: 64px;"
            />
            MB
          </span>
        </label>
      </div>

      {#if error}
        <div class="error-box">{error}</div>
      {/if}

      <div class="modal-actions">
        <button type="button" class="ghost" onclick={onClose} disabled={busy}>Close</button>
        <button type="submit" class="primary" disabled={busy}>
          {busy ? 'Scanning…' : 'Scan project'}
        </button>
      </div>
    </form>

    {#if report}
      <hr style="margin: 18px 0; border: none; border-top: 1px solid #2a2a2a;" />

      <div class="scan-stats">
        <div><strong>Root:</strong> <code>{report.root_path}</code></div>
        <div class="muted" style="font-size: 12px; margin-top: 4px;">
          Scanned {report.scanned_files} file{report.scanned_files === 1 ? '' : 's'} ·
          skipped {report.skipped_binary} binary,
          {report.skipped_oversize} oversize,
          {report.skipped_unreadable} unreadable ·
          {fmtElapsed(report.elapsed_ms)}
          {#if report.respect_gitignore}· respecting .gitignore{/if}
        </div>
      </div>

      {#if report.detections.length === 0}
        <div class="scan-empty">
          <h3>No exposed secrets found.</h3>
          <p class="muted">
            Holster scanned this project and found no matches against the 22 detector patterns.
            This is the all-clear state — no rotation needed.
          </p>
        </div>
      {:else}
        <div class="scan-summary">
          <div class="summary-row">
            <strong>By risk:</strong>
            {#each ['critical', 'high', 'medium', 'low'] as risk}
              {#if report.summary_by_risk[risk]}
                <span class="chip {riskClass(risk as ScanRiskLevel)}">
                  {risk}: {report.summary_by_risk[risk]}
                </span>
              {/if}
            {/each}
          </div>
          <div class="summary-row">
            <strong>By detector:</strong>
            {#each report.summary_by_detector as s (s.detector_id)}
              <span class="chip">
                {s.display_name}: {s.count}
              </span>
            {/each}
          </div>
        </div>

        <div class="scan-filters">
          <label>
            Risk:
            <select bind:value={filterRisk}>
              <option value="all">all</option>
              <option value="critical">critical</option>
              <option value="high">high</option>
              <option value="medium">medium</option>
              <option value="low">low</option>
            </select>
          </label>
          <label>
            Provider:
            <select bind:value={filterProvider}>
              <option value="all">all</option>
              {#each providersInReport as p}
                <option value={p}>{p}</option>
              {/each}
            </select>
          </label>
          <label>
            Git status:
            <select bind:value={filterGitTracked}>
              <option value="all">all</option>
              <option value="tracked">tracked by git (committed)</option>
              <option value="untracked">untracked / gitignored</option>
            </select>
          </label>
          <span class="muted" style="font-size: 12px;">
            Showing {filteredDetections.length} of {report.detections.length}
          </span>
        </div>

        <div class="findings-list">
          {#each filteredDetections as d, i (i)}
            <div class="finding {riskClass(d.risk_level)}">
              <div class="finding-head">
                <span class="chip {riskClass(d.risk_level)}">{d.risk_level}</span>
                <strong>{d.display_name}</strong>
                <span class="provider-badge">{d.provider}</span>
                {#if d.git_tracked === true}
                  <span class="chip warn">tracked by git</span>
                {:else if d.git_tracked === false}
                  <span class="chip muted-chip">untracked / gitignored</span>
                {/if}
              </div>
              <div class="finding-body">
                <div>
                  <code>{d.file_path ?? '(unknown path)'}</code>:<strong>{d.line_number}</strong>
                </div>
                <div class="redacted">
                  Match: <code>{d.redacted_preview}</code>
                </div>
                <div class="action muted">{d.recommended_action}</div>
                {#if d.rotation_url || d.docs_url}
                  <div class="links">
                    {#if d.rotation_url}<a href={d.rotation_url} target="_blank" rel="noreferrer">Rotate</a>{/if}
                    {#if d.docs_url}<a href={d.docs_url} target="_blank" rel="noreferrer">Docs</a>{/if}
                  </div>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .scan-stats {
    margin-bottom: 12px;
  }
  .scan-empty {
    text-align: center;
    padding: 24px 12px;
    border: 1px dashed #2a4a2a;
    border-radius: 6px;
    background: #0d1f0d;
  }
  .scan-empty h3 {
    margin: 0 0 8px 0;
    color: #6fbf73;
  }
  .scan-summary {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 14px;
  }
  .summary-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: center;
  }
  .chip {
    display: inline-block;
    padding: 2px 8px;
    border-radius: 12px;
    background: #2a2a2a;
    font-size: 12px;
    line-height: 18px;
  }
  .chip.risk-critical {
    background: #4a1818;
    color: #ff8a8a;
  }
  .chip.risk-high {
    background: #3f2a18;
    color: #ffb066;
  }
  .chip.risk-medium {
    background: #2f3018;
    color: #d8dd66;
  }
  .chip.risk-low {
    background: #1a2a3a;
    color: #88c2ff;
  }
  .chip.warn {
    background: #4a1818;
    color: #ff8a8a;
  }
  .chip.muted-chip {
    background: #1f1f1f;
    color: #888;
  }
  .scan-filters {
    display: flex;
    gap: 14px;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }
  .scan-filters label {
    font-size: 12px;
    color: #aaa;
  }
  .scan-filters select {
    margin-left: 4px;
  }
  .findings-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 50vh;
    overflow-y: auto;
  }
  .finding {
    border-left: 4px solid #2a2a2a;
    padding: 10px 12px;
    background: #161616;
    border-radius: 4px;
  }
  .finding.risk-critical {
    border-left-color: #cc4444;
  }
  .finding.risk-high {
    border-left-color: #cc8844;
  }
  .finding.risk-medium {
    border-left-color: #cccc44;
  }
  .finding.risk-low {
    border-left-color: #4488cc;
  }
  .finding-head {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 6px;
  }
  .finding-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
  }
  .redacted code {
    color: #ffb066;
  }
  .action {
    font-size: 12px;
  }
  .links {
    display: flex;
    gap: 12px;
    font-size: 12px;
    margin-top: 4px;
  }
  .links a {
    color: #88c2ff;
  }
</style>
