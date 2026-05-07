<script lang="ts">
  interface Props {
    onClose: () => void;
    onOpenDoctor: () => void;
  }

  let { onClose, onOpenDoctor }: Props = $props();
  let activePath = $state<'buying' | 'starting' | 'workstation'>('buying');
  let audienceMode = $state<'personal' | 'business'>('personal');
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

  const buyingDecisions = [
    {
      title: 'Use the computer you have',
      signal: 'You are still learning prompts, writing, planning, research, images, or basic automation.',
      recommendation: 'Start with a browser-based subscription. Your old laptop is enough for the first useful month.',
      wait: 'Wait on a new AI computer until a repeated workflow is blocked by speed, memory, storage, or local privacy needs.'
    },
    {
      title: 'Buy a normal strong computer',
      signal: 'You want a daily work machine for documents, browser tools, video calls, images, and light coding.',
      recommendation: 'Prioritize RAM, storage, screen comfort, and warranty before chasing AI marketing.',
      wait: 'Do not pay a premium for vague "AI PC" claims if your work will mainly happen in ChatGPT, Claude, Gemini, or web apps.'
    },
    {
      title: 'Prepare for API tools',
      signal: 'You have a tool, agent, or workflow that explicitly asks for provider keys.',
      recommendation: 'Set budgets, turn on 2FA, create the smallest needed key, label it clearly, and store it in Holster.',
      wait: 'Do not open API billing just because a tutorial says to. API means software can spend from your account.'
    },
    {
      title: 'Consider a local AI workstation',
      signal: 'You need privacy, local files, code agents, repeated automation, or model experiments that justify complexity.',
      recommendation: 'Buy for the real workload: RAM, GPU, thermals, storage, backup, and support matter more than buzzwords.',
      wait: 'Do not make this the first step. Graduate into local power after you know what you will run and why.'
    }
  ];

  const modeDetails = $derived(audienceMode === 'personal'
    ? {
        label: 'Personal setup',
        title: 'Start simple, protect your accounts, and avoid surprise API bills.',
        copy: 'Best for family, friends, authors, creators, and curious beginners who want AI help without turning their computer into a science project.',
        checkpoints: ['One paid subscription first', '2FA on important accounts', 'No loose API keys', 'Upgrade only after a repeated workflow'],
        demo: [
          {
            title: 'The expensive mistake',
            copy: 'A beginner buys hardware or opens API billing before they know what they actually need.'
          },
          {
            title: 'The safer first month',
            copy: 'Buildbelt points them to one predictable subscription, basic account safety, and clear rules for when not to create keys.'
          },
          {
            title: 'The upgrade moment',
            copy: 'When a real workflow appears, Holster stores keys locally and Safe Share checks projects before AI handoff.'
          }
        ]
      }
    : {
        label: 'Business setup',
        title: 'Give the team a safe AI starting line before tools, keys, and client files spread everywhere.',
        copy: 'Best for owners, managers, and small teams that need approved tools, account controls, billing guardrails, and safer project handoff.',
        checkpoints: ['Approved AI tools list', '2FA and recovery codes', 'Named billing owner', 'Safe Share before client or contractor handoff'],
        demo: [
          {
            title: 'The scattered rollout',
            copy: 'Staff try random AI tools, create unmanaged accounts, and move client files without a shared safety path.'
          },
          {
            title: 'The approved starting line',
            copy: 'Buildbelt defines the first tools, account controls, billing owner, and what must stay off limits.'
          },
          {
            title: 'The safe handoff',
            copy: 'Holster keeps API keys owned and local, while Safe Share checks work before it leaves the machine.'
          }
        ]
      });

  const journeyStages = [
    {
      path: 'buying',
      label: 'Decide',
      title: 'Before spending',
      copy: 'Choose subscription, API, or hardware with fewer surprises.'
    },
    {
      path: 'starting',
      label: 'Learn',
      title: 'First $20 month',
      copy: 'Build useful AI habits before opening metered billing.'
    },
    {
      path: 'workstation',
      label: 'Protect',
      title: 'Safe workstation',
      copy: 'Use Holster when keys, files, agents, or handoff enter the workflow.'
    }
  ] as const;

  const visibleSteps = $derived(activePath === 'starting' ? startingLessons : workstationSteps);
  const currentLesson = $derived(startingLessons[activeLesson]);
  const pathLabel = $derived(activePath === 'buying'
    ? 'Pre-purchase'
    : activePath === 'starting'
      ? 'First $20 month'
      : 'Workstation');
  const nextMove = $derived(activePath === 'buying'
    ? (audienceMode === 'personal'
      ? 'Start with one subscription and use the computer you already have.'
      : 'Name the approved tools, billing owner, and account rules before buying hardware.')
    : activePath === 'starting'
      ? 'Finish the five lessons, then decide whether an API key is truly needed.'
      : 'Run Safe Share Doctor before adding keys or handing files to an AI tool.');
  const doctorTiming = $derived(activePath === 'workstation'
    ? 'Run now before project handoff.'
    : 'Run when you have a real project folder, API key, or agent workflow.');

  function setPath(path: 'buying' | 'starting' | 'workstation') {
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
          Buildbelt helps you choose the right first step before hardware,
          API billing, keys, agent tools, or local workstation complexity.
        </p>
      </div>
      <div class="local-status-card" aria-label="Local trust status">
        <div class="status-ring" aria-hidden="true">N</div>
        <div>
          <span>Local trust</span>
          <strong>Protected on this machine</strong>
          <p>Holster keeps keys and handoff checks behind the local vault boundary.</p>
        </div>
        <dl>
          <div>
            <dt>Cloud sync</dt>
            <dd>Off</dd>
          </div>
          <div>
            <dt>Vault</dt>
            <dd>Local</dd>
          </div>
          <div>
            <dt>Handoff</dt>
            <dd>Scan first</dd>
          </div>
        </dl>
      </div>
    </section>

    <section class="buildbelt-mode" aria-label="Buildbelt setup mode">
      <div>
        <span>{modeDetails.label}</span>
        <strong>{modeDetails.title}</strong>
        <p>{modeDetails.copy}</p>
      </div>
      <div class="mode-switch" role="group" aria-label="Personal or business mode">
        <button
          type="button"
          class:active={audienceMode === 'personal'}
          onclick={() => (audienceMode = 'personal')}
        >
          Personal
        </button>
        <button
          type="button"
          class:active={audienceMode === 'business'}
          onclick={() => (audienceMode = 'business')}
        >
          Business
        </button>
      </div>
    </section>

    <section class="journey-rail" aria-label="Buildbelt guided journey">
      {#each journeyStages as stage, index}
        <button
          type="button"
          class:active={activePath === stage.path}
          onclick={() => setPath(stage.path)}
        >
          <span>{index + 1}</span>
          <div>
            <small>{stage.label}</small>
            <strong>{stage.title}</strong>
            <p>{stage.copy}</p>
          </div>
        </button>
      {/each}
    </section>

    <section class="buildbelt-paths" aria-label="Buildbelt setup paths">
      <button
        type="button"
        class:active={activePath === 'buying'}
        onclick={() => setPath('buying')}
      >
        <span>Before I spend money</span>
        <strong>Should I buy an AI computer?</strong>
        <small>Decide whether to use what you have, subscribe first, prepare API tools, or buy hardware.</small>
      </button>
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

    <section
      class:buildbelt-content={activePath === 'workstation'}
      class:buildbelt-walkthrough={activePath === 'starting'}
      class:buildbelt-buying={activePath === 'buying'}
    >
      {#if activePath === 'buying'}
        <article class="buying-guide">
          <div class="panel-head">
            <h3>Before you buy</h3>
            <span>decision guide</span>
          </div>
          <div class="buying-verdict">
            <span>Buildbelt recommendation</span>
            <strong>{audienceMode === 'personal' ? 'Subscribe first. Buy later with a real workflow.' : 'Approve tools first. Buy hardware after the workflow is proven.'}</strong>
            <p>
              {audienceMode === 'personal'
                ? 'Most beginners do not need an expensive AI computer on day one. They need one predictable AI month, account safety, and a clear reason to graduate into APIs or local tools.'
                : 'Most small teams do not need every employee opening their own tools and keys. They need an approved starting stack, a billing owner, account safety, and a clear handoff process.'}
            </p>
          </div>
          <div class="mode-checkpoints">
            {#each modeDetails.checkpoints as item}
              <span>{item}</span>
            {/each}
          </div>
          <div class="demo-flow" aria-label="Founder demo flow">
            <div class="panel-head">
              <h3>Three-minute demo</h3>
              <span>{audienceMode}</span>
            </div>
            <div>
              {#each modeDetails.demo as step, index}
                <section>
                  <span>{index + 1}</span>
                  <strong>{step.title}</strong>
                  <p>{step.copy}</p>
                </section>
              {/each}
            </div>
          </div>
          <div class="setup-summary" aria-label="Current setup summary">
            <div>
              <span>Current plan</span>
              <strong>{pathLabel} / {modeDetails.label}</strong>
              <p>{nextMove}</p>
            </div>
            <div>
              <span>Doctor timing</span>
              <strong>When ready</strong>
              <p>{doctorTiming}</p>
            </div>
          </div>
          <div class="buying-decisions">
            {#each buyingDecisions as item}
              <section>
                <h4>{item.title}</h4>
                <dl>
                  <div>
                    <dt>Good fit when</dt>
                    <dd>{item.signal}</dd>
                  </div>
                  <div>
                    <dt>Do this</dt>
                    <dd>{item.recommendation}</dd>
                  </div>
                  <div>
                    <dt>Wait on this</dt>
                    <dd>{item.wait}</dd>
                  </div>
                </dl>
              </section>
            {/each}
          </div>
        </article>
      {:else if activePath === 'starting'}
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
          <div class="setup-summary compact-summary" aria-label="Current setup summary">
            <div>
              <span>Current plan</span>
              <strong>{pathLabel} / {modeDetails.label}</strong>
              <p>{nextMove}</p>
            </div>
            <div>
              <span>Doctor timing</span>
              <strong>When ready</strong>
              <p>{doctorTiming}</p>
            </div>
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
          <div class="setup-summary compact-summary" aria-label="Current setup summary">
            <div>
              <span>Current plan</span>
              <strong>{pathLabel} / {modeDetails.label}</strong>
              <p>{nextMove}</p>
            </div>
            <div>
              <span>Doctor timing</span>
              <strong>Before handoff</strong>
              <p>{doctorTiming}</p>
            </div>
          </div>
        </article>
      {/if}

      <aside class="buildbelt-side">
        <article>
          <span>{audienceMode === 'personal' ? 'Subscription first' : 'Approved tools first'}</span>
          <p>{audienceMode === 'personal' ? 'Use a predictable monthly tool before you let software spend from an API account.' : 'Choose the tools your team may use before keys, browser extensions, and client files scatter.'}</p>
        </article>
        <article>
          <span>{audienceMode === 'personal' ? 'API keys are billing keys' : 'Keys need owners'}</span>
          <p>{audienceMode === 'personal' ? 'Create keys only when a tool needs them, then store them behind the local vault boundary.' : 'Every key should have a provider, budget, owner, purpose, and local storage plan.'}</p>
        </article>
        <article>
          <span>Safe Share before handoff</span>
          <p>{audienceMode === 'personal' ? 'Scan projects before sharing folders with AI tools, agents, or contractors.' : 'Scan projects before client work, staff folders, or contractor handoffs leave the machine.'}</p>
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
