<script lang="ts">
  interface Props {
    onClose: () => void;
    onEnvExample: () => void;
    onGitignore: () => void;
    onExportProfile: () => void;
    exportDisabled?: boolean;
  }
  let {
    onClose,
    onEnvExample,
    onGitignore,
    onExportProfile,
    exportDisabled = false
  }: Props = $props();

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="modal-backdrop" role="presentation" onclick={onBackdropClick}>
  <div class="modal tools-modal" role="dialog" aria-modal="true" aria-labelledby="tools-title">
    <header class="dialog-header">
      <div>
        <p class="eyebrow">Project bootstrap</p>
        <h2 id="tools-title">Project Tools</h2>
        <p class="dialog-subtitle">
          Operations that prepare a project for AI-agent handoff. Each tool writes
          atomically and respects existing files.
        </p>
      </div>
      <button type="button" class="dialog-close" onclick={onClose} aria-label="Close">×</button>
    </header>

    <ul class="tool-list">
      <li>
        <button type="button" class="tool-launcher" onclick={onEnvExample}>
          <span class="tool-icon" aria-hidden="true">⌗</span>
          <span class="tool-body">
            <span class="tool-title">Generate .env.example</span>
            <span class="tool-sub">
              Build a committable template listing the env-var names a project needs,
              with placeholder values only. Real secrets stay in the vault.
            </span>
          </span>
        </button>
      </li>
      <li>
        <button type="button" class="tool-launcher" onclick={onGitignore}>
          <span class="tool-icon" aria-hidden="true">⊘</span>
          <span class="tool-body">
            <span class="tool-title">Review .gitignore</span>
            <span class="tool-sub">
              Audit the current .gitignore and atomically append credential-file
              patterns. Existing lines are never removed.
            </span>
          </span>
        </button>
      </li>
      <li>
        <button
          type="button"
          class="tool-launcher"
          onclick={onExportProfile}
          disabled={exportDisabled}
          title={exportDisabled ? 'Add at least one key to the vault first' : ''}
        >
          <span class="tool-icon" aria-hidden="true">↗</span>
          <span class="tool-body">
            <span class="tool-title">Export agent runtime profile</span>
            <span class="tool-sub">
              Generate a runtime profile (Generic / OpenClaw / Claude Code / Codex /
              Hermes) so agents read credentials at process start, not from disk.
            </span>
          </span>
        </button>
      </li>
    </ul>
  </div>
</div>

<style>
  .tools-modal {
    max-width: 640px;
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
    margin-bottom: var(--spacing-lg, 18px);
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

  .tool-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .tool-launcher {
    width: 100%;
    display: flex;
    align-items: flex-start;
    gap: 14px;
    text-align: left;
    padding: 16px;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.04);
    color: #e8edf6;
    cursor: pointer;
    transition: border-color 120ms ease, background 120ms ease, transform 120ms ease;
  }

  .tool-launcher:hover:not(:disabled) {
    border-color: var(--accent, #f1b85b);
    background: rgba(255, 255, 255, 0.06);
    transform: translateY(-1px);
  }

  .tool-launcher:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .tool-icon {
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: rgba(241, 184, 91, 0.16);
    color: var(--accent, #f1b85b);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    font-weight: 600;
  }

  .tool-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }

  .tool-title {
    font-weight: 600;
    font-size: 15px;
    color: #ffffff;
  }

  .tool-sub {
    font-size: 12px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.65);
  }
</style>
