<script lang="ts">
  interface Props {
    onClose: () => void;
    onOpenDoctor: () => void;
  }

  let { onClose, onOpenDoctor }: Props = $props();
  let activePath = $state<'starting' | 'workstation'>('starting');
  let activeLesson = $state(0);

  const startingLessons = [
    {
      title: 'Try one subscription first',
      copy: 'Use a predictable monthly AI subscription for real tasks before buying hardware or opening API billing.',
      means: 'A subscription is the safest first lane because the cost is predictable and the tool is made for a person sitting at the computer.',
      action: 'Pick one main assistant, use it daily for writing, planning, research, files, or images, and write down where it actually saves time.',
      hold: 'Do not buy an AI computer, open multiple paid accounts, or install agent tools until you know what job you want AI to do.'
    },
    {
      title: 'Understand subscription vs API',
      copy: 'Subscriptions are for humans using an app. APIs are for software and agents, and they can spend per call.',
      means: 'An API lets software talk to an AI model directly. That is powerful, but it usually charges by usage instead of one flat monthly fee.',
      action: 'Learn this sentence first: subscription means I use the app; API means software spends from my account.',
      hold: 'Do not paste API keys into random tools or let an agent run unattended while you are still learning what usage costs.'
    },
    {
      title: 'Do not create keys too early',
      copy: 'An API key is a billing key. Create one only when a tool or workflow truly needs software access.',
      means: 'A key is not just a password. It can give a tool permission to spend money from your AI billing account.',
      action: 'When a tool asks for a key, pause and ask what provider, what budget, what job, and where the key will be stored.',
      hold: 'Do not save keys in notes, screenshots, chats, repo files, email, or shared folders.'
    },
    {
      title: 'Watch for unattended agents',
      copy: 'Agents can loop or keep working. Learn the cost model before you let anything run on metered billing.',
      means: 'Agents can call models many times in the background. A small mistake can become real usage before you notice.',
      action: 'Start with short, visible sessions. Stop the run when the job is unclear, and check billing after experiments.',
      hold: 'Do not leave agents running overnight, connect them to sensitive folders, or give them broad write access during your first month.'
    },
    {
      title: 'Graduate when there is a real workflow',
      copy: 'Move from subscription to API to local workstation only when a repeated job justifies the complexity.',
      means: 'The upgrade path is not more tools. It is a repeated task worth protecting, automating, or running locally.',
      action: 'Choose one workflow worth improving, then use Buildbelt/Holster to add account safety, key storage, and Safe Share checks.',
      hold: 'Do not chase every model, extension, or local install before your first useful workflow is obvious.'
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

  const visibleSteps = $derived(activePath === 'starting' ? startingLessons : workstationSteps);
  const currentLesson = $derived(startingLessons[activeLesson]);

  function setPath(path: 'starting' | 'workstation') {
    activePath = path;
    if (path === 'starting') activeLesson = 0;
  }

  function nextLesson() {
    activeLesson = Math.min(activeLesson + 1, startingLessons.length - 1);
  }

  function previousLesson() {
    activeLesson = Math.max(activeLesson - 1, 0);
  }

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
        <div class="nauta-lockup" aria-label="NautaAI">
          <div class="nauta-mark">N</div>
          <div>
            <p class="eyebrow">Buildbelt by NautaAI</p>
            <span>Local AI setup companion</span>
          </div>
        </div>
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
        onclick={() => setPath('starting')}
      >
        <span>I am just getting started</span>
        <strong>First $20 AI Month</strong>
        <small>Learn with one predictable subscription before API billing or new hardware.</small>
      </button>
      <button
        type="button"
        class:active={activePath === 'workstation'}
        onclick={() => setPath('workstation')}
      >
        <span>I already have an AI computer</span>
        <strong>Workstation setup</strong>
        <small>Turn this machine into a safer AI workstation step by step.</small>
      </button>
    </section>

    <section class:buildbelt-content={activePath === 'workstation'} class:buildbelt-walkthrough={activePath === 'starting'}>
      {#if activePath === 'starting'}
        <article class="buildbelt-lesson-nav">
          <div class="panel-head">
            <h3>First $20 AI Month</h3>
            <span>{startingLessons.length} lessons</span>
          </div>
          <div class="lesson-list" role="list">
            {#each startingLessons as step, index}
              <button
                type="button"
                class:active={activeLesson === index}
                onclick={() => (activeLesson = index)}
              >
                <span>{index + 1}</span>
                <strong>{step.title}</strong>
                <small>{step.copy}</small>
              </button>
            {/each}
          </div>
        </article>

        <article class="buildbelt-lesson-card">
          <div class="lesson-progress">
            <span>Lesson {activeLesson + 1} of {startingLessons.length}</span>
            <div aria-hidden="true">
              {#each startingLessons as _, index}
                <i class:active={index <= activeLesson}></i>
              {/each}
            </div>
          </div>
          <h3>{currentLesson.title}</h3>
          <p class="lesson-lead">{currentLesson.copy}</p>

          <div class="lesson-detail-grid">
            <section>
              <span>What this means</span>
              <p>{currentLesson.means}</p>
            </section>
            <section>
              <span>Do this now</span>
              <p>{currentLesson.action}</p>
            </section>
            <section class="lesson-warning">
              <span>Do not do yet</span>
              <p>{currentLesson.hold}</p>
            </section>
          </div>

          <div class="lesson-actions">
            <button type="button" class="ghost" onclick={previousLesson} disabled={activeLesson === 0}>Back</button>
            {#if activeLesson < startingLessons.length - 1}
              <button type="button" class="primary" onclick={nextLesson}>Next lesson</button>
            {:else}
              <button type="button" class="primary" onclick={() => setPath('workstation')}>Show workstation setup</button>
            {/if}
          </div>
        </article>
      {:else}
        <article class="buildbelt-checklist">
          <div class="panel-head">
            <h3>Workstation path</h3>
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
      {/if}

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
