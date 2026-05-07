<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    exportRuntimeProfile,
    listAgentProfiles,
    type AgentProfile,
    type KeyMetadataDto,
    type RuntimeExportReport
  } from '$lib/api';

  interface Props {
    keys: KeyMetadataDto[];
    onClose: () => void;
    onExported: (message: string) => void;
    onSessionExpired: () => void;
  }
  let { keys, onClose, onExported, onSessionExpired }: Props = $props();

  let agentProfiles = $state<AgentProfile[]>([]);
  let selectedProfileId = $state<string>('generic');
  let userOverroteFilename = $state(false);
  let userOverroteProfileName = $state(false);

  let selected = $state<Record<string, boolean>>({});
  let profileName = $state('generic');
  let targetDir = $state('');
  let filename = $state('.env');
  let backupExisting = $state(true);
  let updateGitignore = $state(true);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let preview = $state<RuntimeExportReport | null>(null);

  const selectedIds = $derived(keys.filter((k) => selected[k.id]).map((k) => k.id));
  const activeProfile = $derived(
    agentProfiles.find((p) => p.id === selectedProfileId) ?? null
  );

  $effect(() => {
    listAgentProfiles()
      .then((list) => {
        agentProfiles = list;
        // Apply the default profile (generic) to seed filename + profile_name
        // unless the user has already typed something.
        const generic = list.find((p) => p.id === 'generic');
        if (generic) {
          if (!userOverroteFilename) filename = generic.default_filename;
          if (!userOverroteProfileName) profileName = generic.id;
        }
      })
      .catch((e) => {
        // Non-fatal: dialog still works without the profile presets.
        // Surface the error so we know something's wrong, but don't block.
        error = `could not load agent profiles: ${e instanceof Error ? e.message : String(e)}`;
      });
  });

  function applyProfile(id: string) {
    selectedProfileId = id;
    const p = agentProfiles.find((pp) => pp.id === id);
    if (!p) return;
    // Picks only — never overwrite user-typed values. The user can clear
    // their override (set the field back to empty) to re-enable prefill.
    if (!userOverroteFilename || filename.trim() === '') {
      filename = p.default_filename;
      userOverroteFilename = false;
    }
    if (!userOverroteProfileName || profileName.trim() === '') {
      profileName = p.id;
      userOverroteProfileName = false;
    }
    // Selecting a profile invalidates any prior preview since filename
    // and profile_name may have changed.
    preview = null;
  }

  function onFilenameInput() {
    userOverroteFilename = true;
  }
  function onProfileNameInput() {
    userOverroteProfileName = true;
  }

  function sessionExpired(msg: string) {
    return msg.toLowerCase().includes('session expired') || msg.toLowerCase().includes('session is invalid');
  }

  async function chooseTargetFolder() {
    error = null;
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Choose runtime export folder'
      });
      if (typeof selected === 'string') {
        targetDir = selected;
        preview = null;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function runPreview(e?: SubmitEvent) {
    e?.preventDefault();
    error = null;
    preview = null;
    if (selectedIds.length === 0) {
      error = 'Select at least one key.';
      return;
    }
    if (!targetDir.trim()) {
      error = 'Target folder is required.';
      return;
    }
    busy = true;
    try {
      preview = await exportRuntimeProfile({
        key_ids: selectedIds,
        target_dir: targetDir.trim(),
        filename: filename.trim() || null,
        profile_name: profileName.trim() || null,
        target: 'env_file',
        dry_run: true,
        backup_existing: backupExisting,
        update_gitignore: updateGitignore
      });
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

  async function confirmExport() {
    error = null;
    if (!preview) {
      error = 'Preview before exporting.';
      return;
    }
    busy = true;
    try {
      const report = await exportRuntimeProfile({
        key_ids: selectedIds,
        target_dir: targetDir.trim(),
        filename: filename.trim() || null,
        profile_name: profileName.trim() || null,
        target: 'env_file',
        dry_run: false,
        backup_existing: backupExisting,
        update_gitignore: updateGitignore
      });
      onExported(`Exported ${report.key_count} key${report.key_count === 1 ? '' : 's'} to ${report.target_path}`);
      onClose();
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

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="modal-backdrop" role="presentation" onclick={onBackdropClick}>
  <div class="modal wide-modal" role="dialog" aria-modal="true" aria-labelledby="export-title">
    <h2 id="export-title">Runtime export</h2>
    <p class="muted" style="margin-bottom: 18px;">
      Create a selected-key .env profile for OpenClaw, Hermes, Codex, Claude Code, or a project folder.
    </p>
    <p class="muted" style="margin-bottom: 18px; font-size: 12px;">
      Tip: key <em>labels</em> are recorded in the export audit log (names + paths only, never values).
      Don't put secret values in labels — keep labels descriptive (e.g. "Anthropic Personal", "OpenAI Work").
    </p>

    <form onsubmit={runPreview}>
      <div class="field">
        <label for="agent-profile">Agent profile</label>
        <select
          id="agent-profile"
          value={selectedProfileId}
          onchange={(e) => applyProfile((e.target as HTMLSelectElement).value)}
          disabled={busy || agentProfiles.length === 0}
        >
          {#each agentProfiles as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
        {#if activeProfile}
          <p class="muted" style="font-size: 12px; margin: 6px 0 0;">
            {activeProfile.description}
          </p>
          {#if activeProfile.suggested_env_vars.length > 0}
            <p class="muted" style="font-size: 12px; margin: 6px 0 0;">
              Suggested env vars:
              {#each activeProfile.suggested_env_vars as v, i (v)}<code>{v}</code>{#if i < activeProfile.suggested_env_vars.length - 1}, {/if}{/each}
            </p>
          {/if}
          {#if activeProfile.todo_note}
            <p class="muted" style="font-size: 12px; margin: 6px 0 0; color: #ffb066;">
              ⚠ {activeProfile.todo_note}
            </p>
          {/if}
        {/if}
      </div>

      <div class="field">
        <label for="profile">Profile name (recorded in audit log)</label>
        <input id="profile" type="text" bind:value={profileName} oninput={onProfileNameInput} placeholder="hermes, openclaw, codex, project-name" />
      </div>

      <div class="field two-col">
        <div>
          <label for="target-dir">Target folder</label>
          <div class="path-picker">
            <input id="target-dir" type="text" bind:value={targetDir} placeholder="/Users/admin/my-project" />
            <button type="button" onclick={chooseTargetFolder} disabled={busy}>Browse…</button>
          </div>
        </div>
        <div>
          <label for="filename">File</label>
          <input id="filename" type="text" bind:value={filename} oninput={onFilenameInput} placeholder=".env.local" />
        </div>
      </div>

      <div class="field">
        <div class="field-label">Keys to include</div>
        <div class="key-picker">
          {#each keys as k (k.id)}
            <label class="check-row">
              <input type="checkbox" bind:checked={selected[k.id]} />
              <span>
                <strong>{k.label}</strong>
                <span class="provider-badge">{k.provider}</span>
                <span class="muted">{k.project_tag ?? 'no project'}</span>
              </span>
            </label>
          {/each}
        </div>
      </div>

      <div class="field options-row">
        <label class="check-row inline">
          <input type="checkbox" bind:checked={backupExisting} />
          <span>Back up existing env file</span>
        </label>
        <label class="check-row inline">
          <input type="checkbox" bind:checked={updateGitignore} />
          <span>Add env patterns to .gitignore</span>
        </label>
      </div>

      {#if error}
        <div class="error-box">{error}</div>
      {/if}

      {#if preview}
        <div class="preview-box">
          <div class="preview-head">
            <strong>{preview.target_path}</strong>
            <span class="muted">{preview.key_count} selected</span>
          </div>
          {#if preview.file_exists}
            <p class="muted">Existing file detected. Backup is {backupExisting ? 'enabled' : 'disabled'}.</p>
          {/if}
          {#if preview.git_tracked}
            <div class="error-box">This env file is tracked by git. Holster will refuse to write secrets here.</div>
          {/if}
          <pre>{preview.preview_lines.join('\n')}</pre>
        </div>
      {/if}

      <div class="modal-actions">
        <button type="button" class="ghost" onclick={onClose} disabled={busy}>Cancel</button>
        <button type="submit" disabled={busy}>{busy ? 'Checking…' : 'Preview'}</button>
        <button
          type="button"
          class="primary"
          onclick={confirmExport}
          disabled={busy || !preview || preview.git_tracked}
        >
          Export
        </button>
      </div>
    </form>
  </div>
</div>
