<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    gitignoreAudit,
    gitignoreApply,
    type GitignoreAuditReport,
    type GitignoreApplyReport,
    type GitignoreRuleSet
  } from '$lib/api';

  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  let path = $state('');
  let busy = $state(false);
  let error = $state<string | null>(null);
  let report = $state<GitignoreAuditReport | null>(null);
  let applied = $state<GitignoreApplyReport | null>(null);

  // Per-rule-set, per-line selection state. Initialized once on audit.
  // Map shape: ruleSetId -> Map<line, selected>
  let selected = $state<Record<string, Record<string, boolean>>>({});

  // Re-derive whether a rule set has at least one selected, missing line.
  const setStats = $derived.by(() => {
    if (!report) return {} as Record<string, { selectedNew: number; alreadyPresent: number; total: number }>;
    const out: Record<string, { selectedNew: number; alreadyPresent: number; total: number }> = {};
    for (const set of report.rule_sets) {
      const sel = selected[set.id] ?? {};
      let selectedNew = 0;
      let alreadyPresent = 0;
      for (const r of set.rules) {
        if (r.already_present) alreadyPresent++;
        if (sel[r.line] && !r.already_present) selectedNew++;
      }
      out[set.id] = { selectedNew, alreadyPresent, total: set.rules.length };
    }
    return out;
  });

  const totalNewSelected = $derived(
    Object.values(setStats).reduce((acc, s) => acc + s.selectedNew, 0)
  );

  async function chooseFolder() {
    error = null;
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        title: 'Choose a project folder'
      });
      if (typeof picked === 'string') {
        path = picked;
        report = null;
        applied = null;
        selected = {};
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function runAudit(e?: SubmitEvent) {
    e?.preventDefault();
    error = null;
    report = null;
    applied = null;
    if (!path.trim()) {
      error = 'Choose a project folder first.';
      return;
    }
    busy = true;
    try {
      const r = await gitignoreAudit({ path: path.trim() });
      report = r;
      // Initialize selection state from default_on; pre-check missing lines only.
      const init: Record<string, Record<string, boolean>> = {};
      for (const set of r.rule_sets) {
        const inner: Record<string, boolean> = {};
        for (const rule of set.rules) {
          inner[rule.line] = set.default_on && !rule.already_present;
        }
        init[set.id] = inner;
      }
      selected = init;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  function toggleRuleSet(set: GitignoreRuleSet, on: boolean) {
    if (set.locked_on && !on) return; // can't toggle off the locked set
    const next = { ...(selected[set.id] ?? {}) };
    for (const rule of set.rules) {
      if (rule.already_present) continue; // never propose adding what's there
      next[rule.line] = on;
    }
    selected = { ...selected, [set.id]: next };
  }

  function toggleRule(setId: string, line: string) {
    const next = { ...(selected[setId] ?? {}) };
    next[line] = !next[line];
    selected = { ...selected, [setId]: next };
  }

  function ruleSetAllChecked(set: GitignoreRuleSet): boolean {
    const sel = selected[set.id] ?? {};
    const proposable = set.rules.filter((r) => !r.already_present);
    if (proposable.length === 0) return false;
    return proposable.every((r) => sel[r.line]);
  }

  async function runApply() {
    if (!report) return;
    error = null;
    applied = null;
    busy = true;
    try {
      const selections = report.rule_sets
        .map((set) => {
          const sel = selected[set.id] ?? {};
          const lines = set.rules
            .filter((r) => !r.already_present && sel[r.line])
            .map((r) => r.line);
          return { rule_set_id: set.id, lines };
        })
        .filter((s) => s.lines.length > 0);

      const result = await gitignoreApply({ path: path.trim(), selections });
      applied = result;
      // Re-audit to refresh already_present flags so the user can see the
      // updated state without closing/reopening the dialog.
      if (result.lines_added > 0) {
        const refreshed = await gitignoreAudit({ path: path.trim() });
        report = refreshed;
        const init: Record<string, Record<string, boolean>> = {};
        for (const set of refreshed.rule_sets) {
          const inner: Record<string, boolean> = {};
          for (const rule of set.rules) {
            inner[rule.line] = false;
          }
          init[set.id] = inner;
        }
        selected = init;
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
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
  <div class="modal wide-modal" role="dialog" aria-modal="true" aria-labelledby="gitignore-title">
    <h2 id="gitignore-title">Review .gitignore safety</h2>
    <p class="muted" style="margin-bottom: 14px;">
      Audit a project folder's <code>.gitignore</code> against Holster's safe-defaults
      catalogue. Missing lines are proposed for review. Nothing is written until you confirm.
    </p>
    <p class="muted" style="margin-bottom: 18px; font-size: 12px;">
      Append-only. Existing lines are never removed. Atomic write. No secret values are
      read or displayed by this feature.
    </p>

    <form onsubmit={runAudit}>
      <div class="field">
        <label for="gi-path">Project folder</label>
        <div class="path-picker">
          <input
            id="gi-path"
            type="text"
            bind:value={path}
            placeholder="/Users/admin/my-project"
            disabled={busy}
          />
          <button type="button" onclick={chooseFolder} disabled={busy}>Browse…</button>
        </div>
      </div>

      {#if error}
        <div class="error-box">{error}</div>
      {/if}

      <div class="modal-actions">
        <button type="button" class="ghost" onclick={onClose} disabled={busy}>Close</button>
        <button type="submit" class="primary" disabled={busy}>
          {busy ? 'Auditing…' : report ? 'Re-audit' : 'Audit .gitignore'}
        </button>
      </div>
    </form>

    {#if report}
      <hr style="margin: 18px 0; border: none; border-top: 1px solid #2a2a2a;" />

      <div class="audit-summary">
        <div><strong>Project:</strong> <code>{report.root_path}</code></div>
        <div class="muted" style="font-size: 12px; margin-top: 4px;">
          .gitignore: {report.gitignore_exists ? 'exists' : 'will be created on apply'}
          · {report.existing_line_count} existing line{report.existing_line_count === 1 ? '' : 's'}
          {#if report.project_types.length > 0}
            · detected: {report.project_types.join(' + ')}
          {:else}
            · no language markers detected (generic)
          {/if}
        </div>
      </div>

      {#if applied}
        <div class={applied.lines_added > 0 ? 'apply-success' : 'apply-noop'}>
          {#if applied.lines_added > 0}
            <strong>Applied.</strong> Added {applied.lines_added} line{applied.lines_added === 1 ? '' : 's'} to
            <code>{applied.target_path}</code>{applied.created_new_file ? ' (new file)' : ''}.
          {:else}
            <strong>Nothing to add.</strong> Your .gitignore already covers every selected rule.
          {/if}
        </div>
      {/if}

      <div class="rule-sets">
        {#each report.rule_sets as set (set.id)}
          {@const stats = setStats[set.id] ?? { selectedNew: 0, alreadyPresent: 0, total: 0 }}
          {@const proposable = set.rules.filter((r) => !r.already_present)}
          <div class="rule-set">
            <div class="rule-set-head">
              <label class="set-toggle">
                <input
                  type="checkbox"
                  checked={ruleSetAllChecked(set)}
                  disabled={set.locked_on || proposable.length === 0}
                  onchange={(e) => toggleRuleSet(set, (e.target as HTMLInputElement).checked)}
                />
                <span>
                  <strong>{set.label}</strong>
                  {#if set.auto_detected}<span class="badge auto">auto</span>{/if}
                  {#if set.locked_on}<span class="badge locked">always on</span>{/if}
                </span>
              </label>
              <div class="set-meta muted">
                {stats.selectedNew} to add · {stats.alreadyPresent} already present · {stats.total} total
              </div>
            </div>
            <div class="rule-set-desc muted">{set.description}</div>
            <div class="rule-grid">
              {#each set.rules as rule (rule.line)}
                <label class="rule-row" class:already={rule.already_present}>
                  <input
                    type="checkbox"
                    checked={(selected[set.id] ?? {})[rule.line] ?? false}
                    disabled={rule.already_present}
                    onchange={() => toggleRule(set.id, rule.line)}
                  />
                  <code>{rule.line}</code>
                  {#if rule.already_present}<span class="muted small">already present</span>{/if}
                </label>
              {/each}
            </div>
          </div>
        {/each}
      </div>

      <div class="diff-preview">
        <strong>Preview ({totalNewSelected} new line{totalNewSelected === 1 ? '' : 's'}):</strong>
        {#if totalNewSelected === 0}
          <p class="muted" style="margin: 8px 0 0;">No new lines will be added.</p>
        {:else}
          <pre>{report.rule_sets
            .map((set) => {
              const sel = selected[set.id] ?? {};
              const lines = set.rules
                .filter((r) => !r.already_present && sel[r.line])
                .map((r) => r.line);
              if (lines.length === 0) return null;
              return `${set.header_comment}\n${lines.join('\n')}`;
            })
            .filter(Boolean)
            .join('\n\n')}</pre>
        {/if}
      </div>

      <div class="modal-actions">
        <button type="button" class="ghost" onclick={onClose} disabled={busy}>Close</button>
        <button
          type="button"
          class="primary"
          onclick={runApply}
          disabled={busy || totalNewSelected === 0}
        >
          {busy ? 'Applying…' : `Apply safe .gitignore update (${totalNewSelected})`}
        </button>
      </div>
    {/if}
  </div>
</div>

<style>
  .audit-summary {
    margin-bottom: 14px;
  }
  .apply-success {
    border-left: 4px solid #4a9d6c;
    background: #0e2317;
    color: #b6e5c4;
    padding: 10px 12px;
    border-radius: 4px;
    margin: 10px 0 14px;
    font-size: 13px;
  }
  .apply-noop {
    border-left: 4px solid #888;
    background: #1a1a1a;
    color: #ccc;
    padding: 10px 12px;
    border-radius: 4px;
    margin: 10px 0 14px;
    font-size: 13px;
  }
  .rule-sets {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-height: 42vh;
    overflow-y: auto;
    padding-right: 6px;
  }
  .rule-set {
    border: 1px solid #2a2a2a;
    border-radius: 6px;
    padding: 10px 12px;
    background: #161616;
  }
  .rule-set-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .set-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .rule-set-desc {
    font-size: 12px;
    margin: 4px 0 8px;
  }
  .badge {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 4px;
    margin-left: 6px;
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }
  .badge.auto {
    background: #1f3148;
    color: #88c2ff;
  }
  .badge.locked {
    background: #3f2a18;
    color: #ffb066;
  }
  .set-meta {
    font-size: 11px;
  }
  .rule-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 4px 16px;
  }
  .rule-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .rule-row.already {
    opacity: 0.55;
  }
  .rule-row code {
    font-size: 12px;
  }
  .small {
    font-size: 11px;
  }
  .diff-preview {
    margin: 14px 0;
    border: 1px solid #2a2a2a;
    border-radius: 6px;
    padding: 10px 12px;
    background: #0c0c0c;
  }
  .diff-preview pre {
    margin: 8px 0 0;
    color: #d8dd66;
    font-size: 12px;
    white-space: pre-wrap;
    max-height: 28vh;
    overflow-y: auto;
  }
</style>
