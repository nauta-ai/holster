<script lang="ts">
  import { vaultStatus, type VaultStatusKind } from '$lib/api';
  import '$lib/styles.css';
  import FirstRun from '$lib/views/FirstRun.svelte';
  import Unlock from '$lib/views/Unlock.svelte';
  import Main from '$lib/views/Main.svelte';

  type View = 'loading' | 'first_run' | 'unlock' | 'main';

  let view = $state<View>('loading');
  let vaultPath = $state<string | null>(null);
  let fromTimeout = $state(false);
  let bootError = $state<string | null>(null);

  async function detect() {
    try {
      const r = await vaultStatus();
      vaultPath = r.path;
      view = mapStatus(r.status);
    } catch (e) {
      bootError = e instanceof Error ? e.message : String(e);
      // Fall back to first-run so user has something actionable.
      view = 'first_run';
    }
  }

  function mapStatus(s: VaultStatusKind): View {
    switch (s) {
      case 'no_vault': return 'first_run';
      case 'unlocked': return 'main';
      case 'locked': return 'unlock';
    }
  }

  // Initial boot.
  $effect(() => { detect(); });
</script>

{#if bootError}
  <div class="center-screen">
    <div class="card">
      <h1>Failed to start</h1>
      <p class="error-box">{bootError}</p>
    </div>
  </div>
{:else if view === 'loading'}
  <div class="center-screen">
    <div class="card">
      <p class="muted">Loading…</p>
    </div>
  </div>
{:else if view === 'first_run'}
  <FirstRun
    {vaultPath}
    onCreated={() => { view = 'main'; }}
  />
{:else if view === 'unlock'}
  <Unlock
    {vaultPath}
    {fromTimeout}
    onUnlocked={() => { fromTimeout = false; view = 'main'; }}
  />
{:else if view === 'main'}
  <Main
    onLocked={() => { fromTimeout = false; view = 'unlock'; }}
    onSessionExpired={() => { fromTimeout = true; view = 'unlock'; }}
  />
{/if}
