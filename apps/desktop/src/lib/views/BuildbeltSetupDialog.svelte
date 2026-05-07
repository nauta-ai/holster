<script lang="ts">
  interface Props {
    onClose: () => void;
    onOpenDoctor: () => void;
  }

  let { onClose, onOpenDoctor }: Props = $props();
  let activePath = $state<'starting' | 'workstation'>('starting');

  const startingSteps = [
    {
      title: 'Try one subscription first',
      copy: 'Use a predictable monthly AI subscription for real tasks before buying hardware or opening API billing.'
    },
    {
      title: 'Understand subscription vs API',
      copy: 'Subscriptions are for humans using an app. APIs are for software and agents, and they can spend per call.'
    },
    {
      title: 'Do not create keys too early',
      copy: 'An API key is a billing key. Create one only when a tool or workflow truly needs software access.'
    },
    {
      title: 'Watch for unattended agents',
      copy: 'Agents can loop or keep working. Learn the cost model before you let anything run on metered billing.'
    },
    {
      title: 'Graduate when there is a real workflow',
      copy: 'Move from subscription to API to local workstation only when a repeated job justifies the complexity.'
    }
  ];

  const workstationSteps = [
    {
      title: 'Identify this computer',
      copy: 'Start with the OS and actual job: Windows, Mac, or Linux; writing, images, coding, automation, or local AI.'
    },
    {
      title: 'Secure accounts first',
      copy: 'Use strong passwords, turn on 2FA, and save recovery codes before using AI tools for real work.'
    },
    {
      title: 'Choose first tools',
      copy: 'Pick a small tool stack. ChatGPT, Claude, Gemini, Codex, Cursor, Hermes, and local models do different jobs.'
    },
    {
      title: 'Store keys safely when needed',
      copy: 'If a tool needs API access, label the key clearly and store it in the local vault instead of loose notes.'
    },
    {
      title: 'Run Safe Share before handoff',
      copy: 'Scan folders before pasting, uploading, or handing them to AI tools, agents, or contractors.'
    }
  ];

  const visibleSteps = $derived(activePath === 'starting' ? startingSteps : workstationSteps);

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }

  function openDoctor() {
    onClose();
    onOpenDoctor();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="modal-backdrop" role="presentation" onclick={onBackdropClick}>
  <div class="modal buildbelt-modal" role="dialog" aria-modal="true" aria-labelledby="buildbelt-title">
    <section class="buildbelt-hero">
      <div>
        <p class="eyebrow">Buildbelt by NautaAI</p>
        <h2 id="buildbelt-title">Start your AI setup without the expensive mistakes.</h2>
        <p>
          Buildbelt walks you from first AI curiosity to accounts, keys, cost
          guardrails, safe project sharing, and local workstation readiness.
        </p>
      </div>
      <div class="buildbelt-badge">
        <span>Holster inside</span>
        <strong>Local</strong>
        <small>vault + safety checks</small>
      </div>
    </section>

    <section class="buildbelt-paths" aria-label="Buildbelt setup paths">
      <button
        type="button"
        class:active={activePath === 'starting'}
        onclick={() => (activePath = 'starting')}
      >
        <span>I am just getting started</span>
        <strong>First $20 AI Month</strong>
        <small>Learn with one predictable subscription before API billing or new hardware.</small>
      </button>
      <button
        type="button"
        class:active={activePath === 'workstation'}
        onclick={() => (activePath = 'workstation')}
      >
        <span>I already have an AI computer</span>
        <strong>Workstation setup</strong>
        <small>Turn this machine into a safer AI workstation step by step.</small>
      </button>
    </section>

    <section class="buildbelt-content">
      <article class="buildbelt-checklist">
        <div class="panel-head">
          <h3>{activePath === 'starting' ? 'Beginner path' : 'Workstation path'}</h3>
          <span>{visibleSteps.length} steps</span>
        </div>
        <ol>
          {#each visibleSteps as step}
            <li>
              <strong>{step.title}</strong>
              <p>{step.copy}</p>
            </li>
          {/each}
        </ol>
      </article>

      <aside class="buildbelt-side">
        <article>
          <span>Subscription first</span>
          <p>Use a predictable monthly tool before you let software spend from an API account.</p>
        </article>
        <article>
          <span>API keys are billing keys</span>
          <p>Create keys only when a tool needs them, then store them behind the local vault boundary.</p>
        </article>
        <article>
          <span>Safe Share before handoff</span>
          <p>Scan projects before sharing folders with AI tools, agents, or contractors.</p>
        </article>
      </aside>
    </section>

    <footer class="buildbelt-footer">
      <p>Buildbelt guides the setup. Holster protects the keys and project handoff.</p>
      <div>
        <button type="button" class="ghost" onclick={onClose}>Close</button>
        <button type="button" class="primary" onclick={openDoctor}>Run Safe Share Doctor</button>
      </div>
    </footer>
  </div>
</div>
