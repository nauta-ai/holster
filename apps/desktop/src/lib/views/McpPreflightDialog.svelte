<script lang="ts">
  import {
    analyzeMcpConfig,
    analyzeClaudeDesktopConfig,
    type McpPreflightReport,
    type McpPreflightBatchReport,
    type McpPreflightBatchEntry,
    type PreflightFinding,
    type Verdict,
    type Severity
  } from '$lib/api';

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  type Mode = 'paste' | 'desktop-scan';
  let mode = $state<Mode>('paste');

  // Paste-mode state
  let pasteJson = $state('');
  let pasteName = $state('');
  let pasteReport = $state<McpPreflightReport | null>(null);
  let pasteError = $state<string | null>(null);
  let pasteBusy = $state(false);

  // Claude Desktop scan state
  let batchReport = $state<McpPreflightBatchReport | null>(null);
  let batchError = $state<string | null>(null);
  let batchBusy = $state(false);

  // Sample for the paste-mode hint
  const SAMPLE_CONFIG = JSON.stringify(
    {
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-filesystem', '/Users/me/Documents'],
      env: { NODE_NO_WARNINGS: '1' }
    },
    null,
    2
  );

  async function runPasteAnalysis() {
    pasteBusy = true;
    pasteError = null;
    pasteReport = null;
    try {
      const name = pasteName.trim() || null;
      pasteReport = await analyzeMcpConfig(pasteJson, name);
    } catch (err) {
      pasteError = err instanceof Error ? err.message : String(err);
    } finally {
      pasteBusy = false;
    }
  }

  async function runDesktopScan() {
    batchBusy = true;
    batchError = null;
    batchReport = null;
    try {
      batchReport = await analyzeClaudeDesktopConfig(null);
    } catch (err) {
      batchError = err instanceof Error ? err.message : String(err);
    } finally {
      batchBusy = false;
    }
  }

  function fillSample() {
    pasteJson = SAMPLE_CONFIG;
    pasteName = 'filesystem';
  }

  function verdictTone(v: Verdict): 'safe' | 'caution' | 'risky' {
    return v;
  }

  function verdictLabel(v: Verdict): string {
    return v.charAt(0).toUpperCase() + v.slice(1);
  }

  function severityIcon(s: Severity): string {
    switch (s) {
      case 'risk':
        return '!';
      case 'caution':
        return '·';
      case 'info':
        return 'i';
    }
  }

  function summarizeBatchVerdict(entries: McpPreflightBatchEntry[]): {
    run: Verdict;
    share: Verdict;
  } {
    let run: Verdict = 'safe';
    let share: Verdict = 'safe';
    for (const entry of entries) {
      if (!entry.report) continue;
      if (entry.report.run_verdict === 'risky') run = 'risky';
      else if (entry.report.run_verdict === 'caution' && run !== 'risky') run = 'caution';
      if (entry.report.share_verdict === 'risky') share = 'risky';
      else if (entry.report.share_verdict === 'caution' && share !== 'risky') share = 'caution';
    }
    return { run, share };
  }

  const desktopSummary = $derived.by(() => {
    if (!batchReport || !batchReport.config_found || batchReport.entries.length === 0) {
      return null;
    }
    return summarizeBatchVerdict(batchReport.entries);
  });

  async function copyReport(report: McpPreflightReport) {
    const lines = [
      `Holster MCP Preflight Report`,
      `Server: ${report.server_name ?? '(unnamed)'}`,
      `Command: ${report.raw_command_summary}`,
      ``,
      `Run verdict:   ${verdictLabel(report.run_verdict)}`,
      `Share verdict: ${verdictLabel(report.share_verdict)}`,
      ``,
      `Findings (${report.findings.length}):`
    ];
    for (const f of report.findings) {
      lines.push(`  [${f.severity.toUpperCase()}] ${f.check} (${f.category})`);
      lines.push(`     ${f.message}`);
      if (f.fix_hint) lines.push(`     fix: ${f.fix_hint}`);
    }
    try {
      await navigator.clipboard.writeText(lines.join('\n'));
    } catch {
      // Clipboard write may fail outside Tauri; non-fatal.
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
  <div class="modal mcp-modal" role="dialog" aria-modal="true" aria-labelledby="mcp-title">
    <header class="dialog-header">
      <div>
        <p class="eyebrow">Buildbelt safety check</p>
        <h2 id="mcp-title">MCP Server Preflight</h2>
        <p class="dialog-subtitle">
          Deterministic local analysis. No network calls, no AI loop. Run and share verdicts are
          scored separately because a config that's safe to fire locally can still leak when shared.
        </p>
      </div>
      <button type="button" class="dialog-close" onclick={onClose} aria-label="Close">×</button>
    </header>

  <nav class="preflight-tabs" aria-label="Preflight mode">
    <button
      type="button"
      class="preflight-tab {mode === 'paste' ? 'is-active' : ''}"
      onclick={() => (mode = 'paste')}
    >
      Paste a config
    </button>
    <button
      type="button"
      class="preflight-tab {mode === 'desktop-scan' ? 'is-active' : ''}"
      onclick={() => (mode = 'desktop-scan')}
    >
      Scan Claude Desktop
    </button>
  </nav>

  {#if mode === 'paste'}
    <section class="preflight-panel">
      <div class="field">
        <label for="mcp-name">Server name (optional)</label>
        <input
          id="mcp-name"
          type="text"
          bind:value={pasteName}
          placeholder="filesystem"
          autocomplete="off"
        />
      </div>

      <div class="field">
        <div class="field-header">
          <label for="mcp-json">Server config JSON</label>
          <button type="button" class="link-button" onclick={fillSample}>Use sample</button>
        </div>
        <textarea
          id="mcp-json"
          bind:value={pasteJson}
          placeholder="Paste a single MCP server entry (the value, not the outer mcpServers wrapper)"
          rows="10"
          spellcheck="false"
        ></textarea>
      </div>

      <div class="dialog-actions">
        <button
          type="button"
          class="btn-primary"
          onclick={runPasteAnalysis}
          disabled={pasteBusy || !pasteJson.trim()}
        >
          {pasteBusy ? 'Analyzing…' : 'Run preflight'}
        </button>
      </div>

      {#if pasteError}
        <div class="error-box" role="alert">
          <strong>Could not analyze this config.</strong>
          <p>{pasteError}</p>
        </div>
      {/if}

      {#if pasteReport}
        <div class="preflight-result">
          <div class="preflight-verdicts">
            <div class="verdict-pill verdict-{verdictTone(pasteReport.run_verdict)}">
              <span class="verdict-label">Run</span>
              <span class="verdict-value">{verdictLabel(pasteReport.run_verdict)}</span>
            </div>
            <div class="verdict-pill verdict-{verdictTone(pasteReport.share_verdict)}">
              <span class="verdict-label">Share</span>
              <span class="verdict-value">{verdictLabel(pasteReport.share_verdict)}</span>
            </div>
          </div>

          <p class="preflight-summary">
            <span class="eyebrow">Command preview</span>
            <code>{pasteReport.raw_command_summary}</code>
          </p>

          {#if pasteReport.findings.length === 0}
            <p class="preflight-clean">No findings — this config passes all checks.</p>
          {:else}
            <ul class="preflight-findings">
              {#each pasteReport.findings as finding}
                <li class="preflight-finding sev-{finding.severity}">
                  <div class="finding-row">
                    <span
                      class="finding-icon sev-{finding.severity}"
                      aria-label={finding.severity}
                    >
                      {severityIcon(finding.severity)}
                    </span>
                    <div class="finding-body">
                      <p class="finding-message">
                        <code class="finding-check">{finding.check}</code>
                        <span class="finding-category">{finding.category}</span>
                      </p>
                      <p>{finding.message}</p>
                      {#if finding.fix_hint}
                        <p class="finding-hint"><strong>Fix:</strong> {finding.fix_hint}</p>
                      {/if}
                    </div>
                  </div>
                </li>
              {/each}
            </ul>
          {/if}

          <div class="dialog-actions">
            <button type="button" class="btn-secondary" onclick={() => copyReport(pasteReport)}>
              Copy report
            </button>
          </div>
        </div>
      {/if}
    </section>
  {:else}
    <section class="preflight-panel">
      <p class="preflight-explain">
        Scans <code>~/Library/Application Support/Claude/claude_desktop_config.json</code> and runs
        each entry through the preflight analyzer. Reads only — never modifies the config file.
      </p>

      <div class="dialog-actions">
        <button type="button" class="btn-primary" onclick={runDesktopScan} disabled={batchBusy}>
          {batchBusy ? 'Scanning…' : batchReport ? 'Re-scan' : 'Scan now'}
        </button>
      </div>

      {#if batchError}
        <div class="error-box" role="alert">
          <strong>Could not scan the config file.</strong>
          <p>{batchError}</p>
        </div>
      {/if}

      {#if batchReport}
        <div class="preflight-batch">
          <p class="preflight-path">
            <span class="eyebrow">Config path</span>
            <code>{batchReport.config_path}</code>
          </p>

          {#if !batchReport.config_found}
            <p class="preflight-clean">
              No config file at this path. Either Claude Desktop isn't installed, or no MCP servers
              are registered. You can still use the <strong>Paste a config</strong> tab to test
              entries by hand.
            </p>
          {:else if batchReport.parse_error}
            <div class="error-box">
              <strong>Config file is invalid JSON.</strong>
              <p>{batchReport.parse_error}</p>
            </div>
          {:else if batchReport.entries.length === 0}
            <p class="preflight-clean">
              Config file exists but has no servers under <code>mcpServers</code>.
            </p>
          {:else}
            {#if desktopSummary}
              <div class="preflight-verdicts">
                <div class="verdict-pill verdict-{desktopSummary.run}">
                  <span class="verdict-label">Worst Run</span>
                  <span class="verdict-value">{verdictLabel(desktopSummary.run)}</span>
                </div>
                <div class="verdict-pill verdict-{desktopSummary.share}">
                  <span class="verdict-label">Worst Share</span>
                  <span class="verdict-value">{verdictLabel(desktopSummary.share)}</span>
                </div>
                <p class="preflight-batch-count">
                  {batchReport.entries.length} server{batchReport.entries.length === 1 ? '' : 's'}
                  scanned
                </p>
              </div>
            {/if}

            <ul class="preflight-batch-list">
              {#each batchReport.entries as entry}
                <li class="preflight-batch-row">
                  <div class="batch-row-header">
                    <h3>{entry.server_name}</h3>
                    {#if entry.report}
                      <div class="preflight-verdicts compact">
                        <span class="verdict-pill verdict-{entry.report.run_verdict} compact">
                          Run: {verdictLabel(entry.report.run_verdict)}
                        </span>
                        <span class="verdict-pill verdict-{entry.report.share_verdict} compact">
                          Share: {verdictLabel(entry.report.share_verdict)}
                        </span>
                      </div>
                    {/if}
                  </div>
                  {#if entry.error}
                    <p class="batch-row-error">{entry.error}</p>
                  {:else if entry.report}
                    <p class="batch-row-cmd"><code>{entry.report.raw_command_summary}</code></p>
                    {#if entry.report.findings.length > 0}
                      <ul class="preflight-findings">
                        {#each entry.report.findings as finding}
                          <li class="preflight-finding sev-{finding.severity}">
                            <div class="finding-row">
                              <span class="finding-icon sev-{finding.severity}">
                                {severityIcon(finding.severity)}
                              </span>
                              <div class="finding-body">
                                <p class="finding-message">
                                  <code class="finding-check">{finding.check}</code>
                                  <span class="finding-category">{finding.category}</span>
                                </p>
                                <p>{finding.message}</p>
                                {#if finding.fix_hint}
                                  <p class="finding-hint">
                                    <strong>Fix:</strong>
                                    {finding.fix_hint}
                                  </p>
                                {/if}
                              </div>
                            </div>
                          </li>
                        {/each}
                      </ul>
                    {:else}
                      <p class="preflight-clean compact">No findings.</p>
                    {/if}
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
    </section>
  {/if}
  </div>
</div>

<style>
  .mcp-modal {
    max-width: 760px;
    width: 100%;
    max-height: 90vh;
    overflow-y: auto;
    padding: var(--spacing-xl, 24px);
  }

  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--spacing-md, 12px);
    margin-bottom: var(--spacing-md, 12px);
  }

  .dialog-header h2 {
    margin: 4px 0 6px;
    font-size: 22px;
    color: #ffffff;
  }

  .dialog-subtitle {
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.5;
    margin: 0;
    max-width: 60ch;
  }

  .dialog-close {
    background: transparent;
    border: none;
    font-size: 24px;
    line-height: 1;
    color: rgba(255, 255, 255, 0.55);
    cursor: pointer;
    padding: 4px 10px;
    border-radius: 6px;
    transition: background 120ms ease, color 120ms ease;
  }

  .dialog-close:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #ffffff;
  }

  .preflight-tabs {
    display: flex;
    gap: var(--spacing-sm, 8px);
    margin: var(--spacing-md, 12px) 0 var(--spacing-lg, 16px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }

  .preflight-tab {
    padding: 8px 14px;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: rgba(255, 255, 255, 0.55);
    cursor: pointer;
    font-weight: 600;
    transition: color 120ms ease, border-color 120ms ease;
  }

  .preflight-tab:hover {
    color: #ffffff;
  }

  .preflight-tab.is-active {
    color: #ffffff;
    border-bottom-color: var(--accent, #b8781f);
  }

  .preflight-panel {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md, 12px);
  }

  .preflight-explain {
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.5;
  }

  .field-header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .link-button {
    background: none;
    border: none;
    color: var(--accent, #b8781f);
    cursor: pointer;
    font-size: 12px;
    font-weight: 600;
    text-decoration: underline;
  }

  textarea {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 12px;
    line-height: 1.5;
  }

  .preflight-result,
  .preflight-batch {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md, 12px);
    padding-top: var(--spacing-sm, 8px);
    border-top: 1px solid rgba(255, 255, 255, 0.1);
  }

  .preflight-verdicts {
    display: flex;
    gap: var(--spacing-md, 12px);
    align-items: center;
    flex-wrap: wrap;
  }

  .preflight-verdicts.compact {
    gap: 8px;
  }

  .preflight-batch-count {
    color: rgba(255, 255, 255, 0.6);
    font-size: 12px;
    margin-left: auto;
  }

  .verdict-pill {
    display: inline-flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 14px;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.04);
    color: #ffffff;
    font-weight: 600;
  }

  .verdict-pill.compact {
    flex-direction: row;
    gap: 6px;
    padding: 4px 10px;
    font-size: 12px;
  }

  .verdict-pill .verdict-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: rgba(255, 255, 255, 0.6);
  }

  .verdict-pill .verdict-value {
    font-size: 16px;
  }

  .verdict-pill.compact .verdict-label {
    font-size: 11px;
  }

  .verdict-safe {
    border-color: rgba(95, 200, 130, 0.45);
    background: rgba(47, 122, 74, 0.18);
    color: #8ee0ad;
  }

  .verdict-caution {
    border-color: rgba(245, 180, 90, 0.45);
    background: rgba(184, 120, 31, 0.18);
    color: #f3c172;
  }

  .verdict-risky {
    border-color: rgba(240, 130, 110, 0.5);
    background: rgba(176, 74, 48, 0.22);
    color: #f3a18d;
  }

  .preflight-summary code,
  .preflight-path code,
  .batch-row-cmd code {
    display: block;
    background: rgba(255, 255, 255, 0.06);
    color: #e8edf6;
    padding: 8px 10px;
    border-radius: 6px;
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 12px;
    overflow-x: auto;
  }

  .preflight-clean {
    color: #8ee0ad;
    font-weight: 500;
  }

  .preflight-clean.compact {
    font-size: 13px;
  }

  .preflight-findings {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .preflight-finding {
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
    color: #e8edf6;
  }

  .preflight-finding.sev-risk {
    border-left: 3px solid #b04a30;
  }

  .preflight-finding.sev-caution {
    border-left: 3px solid #b8781f;
  }

  .preflight-finding.sev-info {
    border-left: 3px solid #6c6c78;
  }

  .finding-row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
  }

  .finding-icon {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 12px;
    flex-shrink: 0;
  }

  .finding-icon.sev-risk {
    background: rgba(240, 130, 110, 0.22);
    color: #f3a18d;
  }

  .finding-icon.sev-caution {
    background: rgba(245, 180, 90, 0.22);
    color: #f3c172;
  }

  .finding-icon.sev-info {
    background: rgba(255, 255, 255, 0.12);
    color: #cfd6e3;
  }

  .finding-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .finding-message {
    display: flex;
    gap: 8px;
    align-items: baseline;
    margin: 0 0 2px;
  }

  .finding-check {
    font-family: ui-monospace, SFMono-Regular, monospace;
    font-size: 12px;
    background: rgba(255, 255, 255, 0.08);
    color: #e8edf6;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .finding-category {
    font-size: 10px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.5);
  }

  .finding-hint {
    color: rgba(255, 255, 255, 0.7);
    font-size: 13px;
  }

  .preflight-batch-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .preflight-batch-row {
    padding: 14px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
    color: #e8edf6;
  }

  .batch-row-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    margin-bottom: 8px;
  }

  .batch-row-header h3 {
    margin: 0;
    font-size: 14px;
  }

  .batch-row-error {
    color: #8c3b27;
    font-size: 13px;
  }

  .batch-row-cmd {
    margin-bottom: 8px;
  }
</style>
