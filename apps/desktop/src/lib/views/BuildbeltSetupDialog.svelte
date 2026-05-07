<script lang="ts">
  interface Props {
    onClose: () => void;
    onOpenDoctor: () => void;
    startupMode?: boolean;
    onStartupDone?: () => void;
  }

  let { onClose, onOpenDoctor, startupMode = false, onStartupDone }: Props = $props();
  let activePath = $state<'buying' | 'starting' | 'workstation'>('buying');
  let audienceMode = $state<'personal' | 'business'>('personal');
  let activeLesson = $state(0);
  let startupStep = $state<'chooser' | 'beginner' | 'guidedSignup' | 'businessRollout' | 'oldComputer' | 'buyingSystem' | 'full'>('full');
  let startupInitialized = $state(false);
  let startupAudience = $state<'personal' | 'business' | null>(null);
  let wizardStep = $state(0);
  let selectedSubscription = $state<'chatgpt' | 'claude' | 'gemini'>('chatgpt');
  let copiedPrompt = $state<string | null>(null);
  let signupChecklist = $state({
    account: false,
    firstPrompt: false,
    security: false,
    recovery: false,
    noApi: false
  });
  let businessChecklist = $state({
    owner: false,
    tools: false,
    security: false,
    dataRules: false,
    noStaffApi: false
  });
  let oldComputerChecklist = $state({
    updated: false,
    browser: false,
    security: false,
    storage: false,
    subscription: false
  });
  let buyingSystemChecklist = $state({
    workload: false,
    browserFirst: false,
    security: false,
    specs: false,
    noApi: false
  });

  const startupProgressKey = 'buildbeltStartupProgress';

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

  const startupChoices = [
    {
      id: 'beginner',
      label: 'New person',
      title: 'Start free, upgrade only if useful',
      copy: 'Try one official AI app first. No API keys. No new computer. No agent tools yet.'
    },
    {
      id: 'oldComputer',
      label: 'Somewhat knowledgeable',
      title: 'Use an old computer first',
      copy: 'Turn what you already have into a safe learning machine before spending real money.'
    },
    {
      id: 'buyingSystem',
      label: 'Know what I want',
      title: 'I am buying a system for AI',
      copy: 'Use a clear checklist so the purchase matches your actual AI workload.'
    },
    {
      id: 'business',
      label: 'Business or team',
      title: 'Set up AI for a small team',
      copy: 'Approve tools, billing, account safety, and file rules before everyone starts experimenting.'
    }
  ] as const;

  const starterSubscriptions = [
    {
      id: 'chatgpt',
      label: 'ChatGPT',
      plan: 'Official app account',
      link: 'https://chatgpt.com/',
      bestFor: 'Best first pick for most new users.',
      why: 'Writing, planning, research, files, images, and everyday help in one familiar place.'
    },
    {
      id: 'claude',
      label: 'Claude',
      plan: 'Official app account',
      link: 'https://claude.ai/',
      bestFor: 'Great for long documents and thoughtful writing.',
      why: 'Strong for reading, editing, strategy, and work that needs careful tone.'
    },
    {
      id: 'gemini',
      label: 'Gemini',
      plan: 'Official app account',
      link: 'https://gemini.google.com/',
      bestFor: 'Useful if they already live in Google.',
      why: 'A comfortable path for Gmail, Docs, Drive, Android, and Google account users.'
    }
  ] as const;

  const comparePrompt = 'I am brand new to AI. Ask me five simple questions about what I want help with, then compare ChatGPT, Claude, and Gemini in plain English. Tell me which one I should start with, whether the free plan is enough, and when a paid personal subscription would make sense. Do not recommend API keys yet.';
  const starterPrompts = [
    {
      label: 'Find my use cases',
      prompt: 'Ask me five questions about my work and home life, then suggest three useful ways I can use AI this week.'
    },
    {
      label: 'Try everyday help',
      prompt: 'Help me write one email, plan one task, and summarize one document so I can see what AI is good at.'
    },
    {
      label: 'Learn the basics',
      prompt: 'Explain AI subscriptions, API billing, local AI, and agents like I am brand new. Tell me what to avoid for now.'
    }
  ];
  const businessRolloutPrompt = 'I own or manage a small business and want my team to start using AI safely. Ask me seven questions about our work, client data, staff roles, and budget. Then recommend a simple approved AI tool list, billing owner, 2FA rules, file sharing rules, and what staff should not do yet. Do not recommend staff API keys.';
  const oldComputerPrompt = 'I have an older computer and I want to start using AI without wasting money. Ask me what operating system I have, how old the computer is, how much RAM and storage it has, what browser I use, and what I want AI to help with. Then tell me if I should start with browser AI tools, what to update first, what not to install yet, and when a new computer would actually be worth buying.';
  const buyingSystemPrompt = 'I am thinking about buying a computer for AI. Ask me what I want AI to do: writing, documents, coding, image work, video work, local models, business automation, or agents. Then tell me whether I should keep using browser AI, buy a normal strong computer, or consider a local AI workstation. Explain the tradeoffs in plain English and do not recommend API keys until there is a real workflow.';

  const visibleSteps = $derived(activePath === 'starting' ? startingLessons : workstationSteps);
  const currentSubscription = $derived(
    starterSubscriptions.find((subscription) => subscription.id === selectedSubscription) ?? starterSubscriptions[0]
  );
  const signupReadyToPark = $derived(
    signupChecklist.account
      && signupChecklist.firstPrompt
      && signupChecklist.security
      && signupChecklist.recovery
      && signupChecklist.noApi
  );
  const businessReadyToPark = $derived(
    businessChecklist.owner
      && businessChecklist.tools
      && businessChecklist.security
      && businessChecklist.dataRules
      && businessChecklist.noStaffApi
  );
  const oldComputerReadyToPark = $derived(
    oldComputerChecklist.updated
      && oldComputerChecklist.browser
      && oldComputerChecklist.security
      && oldComputerChecklist.storage
      && oldComputerChecklist.subscription
  );
  const buyingSystemReadyToPark = $derived(
    buyingSystemChecklist.workload
      && buyingSystemChecklist.browserFirst
      && buyingSystemChecklist.security
      && buyingSystemChecklist.specs
      && buyingSystemChecklist.noApi
  );
  const wizardTotal = $derived(startupAudience === 'business' ? 5 : startupAudience === 'personal' ? 6 : 1);
  const modelLabel = $derived(currentSubscription.label);
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

  $effect(() => {
    if (startupInitialized) return;
    loadStartupProgress();
    if (startupMode) {
      if (startupStep === 'full') startupStep = 'chooser';
    } else {
      startupStep = 'full';
    }
    startupInitialized = true;
  });

  $effect(() => {
    if (!startupInitialized) return;
    saveStartupProgress();
  });

  function setPath(path: 'buying' | 'starting' | 'workstation') {
    activePath = path;
    if (path === 'starting') activeLesson = 0;
  }

  function chooseStartup(step: 'beginner' | 'oldComputer' | 'buyingSystem' | 'business') {
    if (step === 'beginner') setPath('starting');
    if (step === 'oldComputer') setPath('buying');
    if (step === 'buyingSystem') setPath('workstation');
    if (step === 'business') {
      audienceMode = 'business';
      setPath('buying');
      startupStep = 'businessRollout';
      return;
    }
    startupStep = step;
  }

  function finishStartup() {
    try {
      localStorage.setItem('buildbeltStartupComplete', 'true');
    } catch {
      // Ignore storage failures; closing the startup still works for this session.
    }
    onStartupDone?.();
    onClose();
  }

  function resetStartupProgress() {
    signupChecklist = {
      account: false,
      firstPrompt: false,
      security: false,
      recovery: false,
      noApi: false
    };
    businessChecklist = {
      owner: false,
      tools: false,
      security: false,
      dataRules: false,
      noStaffApi: false
    };
    oldComputerChecklist = {
      updated: false,
      browser: false,
      security: false,
      storage: false,
      subscription: false
    };
    buyingSystemChecklist = {
      workload: false,
      browserFirst: false,
      security: false,
      specs: false,
      noApi: false
    };
    selectedSubscription = 'chatgpt';
    startupAudience = null;
    wizardStep = 0;
    startupStep = 'chooser';
    try {
      localStorage.removeItem('buildbeltStartupComplete');
      localStorage.removeItem(startupProgressKey);
    } catch {
      // Reset still works for this session if local storage is unavailable.
    }
  }

  function loadStartupProgress() {
    try {
      const raw = localStorage.getItem(startupProgressKey);
      if (!raw) return;
      const progress = JSON.parse(raw);
      if (isStartupStep(progress.startupStep)) startupStep = progress.startupStep;
      if (progress.startupAudience === 'personal' || progress.startupAudience === 'business') startupAudience = progress.startupAudience;
      if (Number.isInteger(progress.wizardStep)) wizardStep = Math.max(0, progress.wizardStep);
      if (progress.selectedSubscription) selectedSubscription = progress.selectedSubscription;
      if (progress.signupChecklist) signupChecklist = { ...signupChecklist, ...progress.signupChecklist };
      if (progress.businessChecklist) businessChecklist = { ...businessChecklist, ...progress.businessChecklist };
      if (progress.oldComputerChecklist) oldComputerChecklist = { ...oldComputerChecklist, ...progress.oldComputerChecklist };
      if (progress.buyingSystemChecklist) buyingSystemChecklist = { ...buyingSystemChecklist, ...progress.buyingSystemChecklist };
    } catch {
      // Corrupt or unavailable startup progress should not block the guide.
    }
  }

  function saveStartupProgress() {
    try {
      localStorage.setItem(startupProgressKey, JSON.stringify({
        startupStep,
        startupAudience,
        wizardStep,
        selectedSubscription,
        signupChecklist,
        businessChecklist,
        oldComputerChecklist,
        buyingSystemChecklist
      }));
    } catch {
      // The guide remains usable without persistence.
    }
  }

  function isStartupStep(value: unknown): value is typeof startupStep {
    return value === 'chooser'
      || value === 'beginner'
      || value === 'guidedSignup'
      || value === 'businessRollout'
      || value === 'oldComputer'
      || value === 'buyingSystem';
  }

  function showFullGuide() {
    startupStep = 'full';
  }

  function chooseWizardAudience(audience: 'personal' | 'business') {
    startupAudience = audience;
    audienceMode = audience;
    wizardStep = 0;
  }

  function nextWizardStep() {
    if (!startupAudience) return;
    wizardStep = Math.min(wizardStep + 1, wizardTotal - 1);
  }

  function previousWizardStep() {
    if (!startupAudience) return;
    if (wizardStep === 0) {
      startupAudience = null;
      return;
    }
    wizardStep = Math.max(wizardStep - 1, 0);
  }

  async function copyPrompt(label: string, prompt: string) {
    try {
      await navigator.clipboard.writeText(prompt);
      copiedPrompt = label;
      setTimeout(() => {
        if (copiedPrompt === label) copiedPrompt = null;
      }, 1800);
    } catch {
      copiedPrompt = null;
    }
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
  <div
    class="modal buildbelt-modal"
    class:startup-modal={startupStep !== 'full'}
    role="dialog"
    aria-modal="true"
    aria-labelledby="buildbelt-title"
  >
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

    {#if startupStep !== 'full'}
      {#if true}
        <section class="startup-wizard" aria-label="Buildbelt startup wizard">
          {#if !startupAudience}
            <div class="wizard-step-label">Step 1</div>
            <h3>Is this for you or a business?</h3>
            <p>Pick one. Buildbelt will show one step at a time.</p>
            <div class="wizard-choice-grid two">
              <button type="button" onclick={() => chooseWizardAudience('personal')}>
                <strong>Personal</strong>
                <span>Start using AI yourself.</span>
              </button>
              <button type="button" onclick={() => chooseWizardAudience('business')}>
                <strong>Business</strong>
                <span>Set rules before a team starts.</span>
              </button>
            </div>
            <div class="wizard-actions">
              <button type="button" class="ghost" onclick={showFullGuide}>Full guide</button>
              <button type="button" class="ghost" onclick={finishStartup}>Skip startup</button>
            </div>
          {:else if startupAudience === 'personal'}
            <div class="wizard-step-label">Step {wizardStep + 2} of {wizardTotal + 1}</div>
            {#if wizardStep === 0}
              <h3>Can AI help you?</h3>
              <p>Yes, if you write, plan, research, summarize, organize, brainstorm, or create.</p>
              <div class="wizard-one-card">
                <strong>Goal</strong>
                <span>Find one real task where AI saves time this week.</span>
              </div>
            {:else if wizardStep === 1}
              <h3>Start simple.</h3>
              <p>Use an official AI app first. Start free. Upgrade only when limits get in the way.</p>
              <div class="wizard-one-card">
                <strong>Do not do yet</strong>
                <span>No API keys. No agents. No new computer.</span>
              </div>
            {:else if wizardStep === 2}
              <h3>Ask AI which one fits you.</h3>
              <p>Copy this into any chat you already use. Then pick the model it recommends.</p>
              <div class="first-prompt-card wizard-prompt" aria-label="Model selection prompt">
                <span>First prompt</span>
                <strong>Help me choose ChatGPT, Claude, or Gemini.</strong>
                <p>{comparePrompt}</p>
                <button type="button" class="ghost prompt-copy" onclick={() => copyPrompt('compare', comparePrompt)}>
                  {copiedPrompt === 'compare' ? 'Copied' : 'Copy prompt'}
                </button>
              </div>
              <div class="wizard-choice-grid three">
                {#each starterSubscriptions as subscription}
                  <button
                    type="button"
                    class:active={selectedSubscription === subscription.id}
                    onclick={() => (selectedSubscription = subscription.id)}
                  >
                    <strong>{subscription.label}</strong>
                    <span>{subscription.bestFor}</span>
                  </button>
                {/each}
              </div>
            {:else if wizardStep === 3}
              <h3>Open {modelLabel}.</h3>
              <p>Create an account or sign in. You handle passwords and payment yourself.</p>
              <a class="primary link-button wizard-link" href={currentSubscription.link} target="_blank" rel="noreferrer">
                Open official {modelLabel} app
              </a>
            {:else if wizardStep === 4}
              <h3>Secure the account.</h3>
              <p>Turn on 2FA or passkey. Save recovery codes. Then stop.</p>
              <div class="wizard-one-card">
                <strong>Still no API keys</strong>
                <span>API billing is for tools and software later.</span>
              </div>
            {:else}
              <h3>Try one prompt.</h3>
              <p>{comparePrompt}</p>
              <button type="button" class="ghost prompt-copy" onclick={() => copyPrompt('compare', comparePrompt)}>
                {copiedPrompt === 'compare' ? 'Copied' : 'Copy prompt'}
              </button>
              <div class="startup-parked compact">
                <strong>Done for now.</strong>
                <p>Use AI for normal work this week. Come back when you know what you want to do repeatedly.</p>
              </div>
            {/if}
            <div class="wizard-actions">
              <button type="button" class="ghost" onclick={previousWizardStep}>Back</button>
              {#if wizardStep < wizardTotal - 1}
                <button type="button" class="primary" onclick={nextWizardStep}>Next</button>
              {:else}
                <button type="button" class="primary" onclick={finishStartup}>Done</button>
              {/if}
            </div>
          {:else}
            <div class="wizard-step-label">Step {wizardStep + 2} of {wizardTotal + 1}</div>
            {#if wizardStep === 0}
              <h3>Can AI help your business?</h3>
              <p>Yes, but start with rules before staff use random tools.</p>
              <div class="wizard-one-card">
                <strong>Goal</strong>
                <span>Let the team test AI without losing control of files, accounts, or cost.</span>
              </div>
            {:else if wizardStep === 1}
              <h3>Pick approved tools.</h3>
              <p>Choose one or two official AI apps. Write down what each is allowed for.</p>
              <div class="wizard-one-card">
                <strong>Keep it small</strong>
                <span>One approved starting tool beats ten unmanaged experiments.</span>
              </div>
            {:else if wizardStep === 2}
              <h3>Name the owner.</h3>
              <p>One person owns billing, recovery codes, admin access, and offboarding.</p>
              <div class="wizard-one-card">
                <strong>Protect access</strong>
                <span>Require 2FA or passkeys for every user.</span>
              </div>
            {:else if wizardStep === 3}
              <h3>Set file rules.</h3>
              <p>Decide what client files, contracts, private notes, and customer data cannot be pasted into AI.</p>
              <button type="button" class="ghost prompt-copy" onclick={() => copyPrompt('business', businessRolloutPrompt)}>
                {copiedPrompt === 'business' ? 'Copied' : 'Copy policy prompt'}
              </button>
            {:else}
              <h3>Park the team here.</h3>
              <p>No staff API keys. No unattended agents. No client folders connected yet.</p>
              <div class="startup-parked compact">
                <strong>Safe starting line.</strong>
                <p>Let the team use approved AI for normal work first. Come back when a repeated workflow is proven.</p>
              </div>
            {/if}
            <div class="wizard-actions">
              <button type="button" class="ghost" onclick={previousWizardStep}>Back</button>
              {#if wizardStep < wizardTotal - 1}
                <button type="button" class="primary" onclick={nextWizardStep}>Next</button>
              {:else}
                <button type="button" class="primary" onclick={finishStartup}>Done</button>
              {/if}
            </div>
          {/if}
        </section>
      {:else if startupStep === 'beginner'}
        <section class="startup-focus" aria-label="Beginner startup">
          <span>Start here</span>
          <h3>Try one official AI app first.</h3>
          <p>
            Start free if that works. Upgrade only when the limits get in the way.
            Your goal is to find one real task AI helps with.
          </p>
          <div class="startup-focus-grid">
            <article>
              <strong>Do now</strong>
              <p>Pick one assistant, use it daily, and write down where it saves time.</p>
            </article>
            <article>
              <strong>Do not do yet</strong>
              <p>Do not create API keys, buy hardware, or leave agents running.</p>
            </article>
            <article>
              <strong>Next checkpoint</strong>
              <p>After a useful workflow appears, come back for key safety and Safe Share.</p>
            </article>
          </div>
          <div class="startup-actions">
            <button type="button" class="ghost" onclick={() => (startupStep = 'chooser')}>Back</button>
            <button type="button" class="ghost" onclick={showFullGuide}>Show details</button>
            <button type="button" class="primary" onclick={() => (startupStep = 'guidedSignup')}>Guide me through signup</button>
          </div>
        </section>
      {:else if startupStep === 'guidedSignup'}
        <section class="startup-focus" aria-label="Guided subscription signup">
          <span>Guided signup</span>
          <h3>Pick one official app, learn the basics, then stop.</h3>
          <p>
            Buildbelt opens the official app and keeps the checklist here. Start free
            if that is enough, upgrade only when the user understands the goal.
          </p>
          <div class="subscription-picker" role="group" aria-label="Choose a first AI subscription">
            {#each starterSubscriptions as subscription}
              <button
                type="button"
                class:active={selectedSubscription === subscription.id}
                onclick={() => (selectedSubscription = subscription.id)}
              >
                <span>{subscription.plan}</span>
                <strong>{subscription.label}</strong>
                <small>{subscription.bestFor}</small>
              </button>
            {/each}
          </div>
          <div class="choice-guide" aria-label="What to choose">
            <article>
              <strong>Start free</strong>
              <p>Create the account, try real tasks, and learn the interface before spending.</p>
            </article>
            <article>
              <strong>Upgrade to paid personal</strong>
              <p>Choose this when they use it daily or need higher limits, files, images, voice, or priority access.</p>
            </article>
            <article>
              <strong>Do not choose API yet</strong>
              <p>API and developer billing are for software tools. That comes later after a real workflow exists.</p>
            </article>
          </div>
          <div class="guided-signup-card">
            <div>
              <span>Recommended next step</span>
              <strong>{currentSubscription.label}</strong>
              <p>{currentSubscription.why}</p>
            </div>
            <ol>
              <li>Open the official site and create or sign in to the account.</li>
              <li>Start free, or choose paid personal if the limits block the goal.</li>
              <li>Turn on 2FA or passkeys before saving real work there.</li>
              <li>Save recovery codes somewhere safe.</li>
              <li>Try the starter prompts below, then park here. No API keys yet.</li>
            </ol>
            <a class="primary link-button" href={currentSubscription.link} target="_blank" rel="noreferrer">
              Open official {currentSubscription.label} app
            </a>
          </div>
          <div class="first-prompt-card" aria-label="First prompt">
            <span>First prompt</span>
            <strong>Use AI to help choose the right subscription.</strong>
            <p>{comparePrompt}</p>
            <button type="button" class="ghost prompt-copy" onclick={() => copyPrompt('compare', comparePrompt)}>
              {copiedPrompt === 'compare' ? 'Copied' : 'Copy prompt'}
            </button>
          </div>
          <div class="prompt-starters" aria-label="Beginner prompt starters">
            <strong>Beginner prompts to try next</strong>
            {#each starterPrompts as item}
              <article>
                <span>{item.label}</span>
                <p>{item.prompt}</p>
                <button type="button" class="ghost prompt-copy" onclick={() => copyPrompt(item.label, item.prompt)}>
                  {copiedPrompt === item.label ? 'Copied' : 'Copy prompt'}
                </button>
              </article>
            {/each}
          </div>
          <div class="startup-checklist" aria-label="Guided signup checklist" onchange={saveStartupProgress}>
            <strong>Park here checklist</strong>
            <label>
              <input type="checkbox" bind:checked={signupChecklist.account} />
              <span>Official app account created or signed in</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={signupChecklist.firstPrompt} />
              <span>First prompt tried inside the app</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={signupChecklist.security} />
              <span>2FA or passkey turned on</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={signupChecklist.recovery} />
              <span>Recovery codes saved somewhere safe</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={signupChecklist.noApi} />
              <span>No API keys or developer billing opened</span>
            </label>
          </div>
          {#if signupReadyToPark}
            <div class="startup-parked" aria-label="Startup parked">
              <strong>You are done for now.</strong>
              <p>Use the app for normal work this week. Come back when there is a repeated workflow worth protecting or automating.</p>
            </div>
          {/if}
          <div class="startup-hold">
            <strong>Stop point</strong>
            <p>
              Use the app for a week on normal work. Do not create API keys,
              install agents, buy hardware, or connect private folders until there is a real workflow.
            </p>
          </div>
          <div class="startup-actions">
            <button type="button" class="ghost" onclick={() => (startupStep = 'beginner')}>Back</button>
            <button type="button" class="primary" onclick={finishStartup}>Done with startup</button>
          </div>
        </section>
      {:else if startupStep === 'businessRollout'}
        <section class="startup-focus" aria-label="Business AI rollout">
          <span>Business setup</span>
          <h3>Approve the AI starting line before the team scatters.</h3>
          <p>
            Buildbelt helps the owner pick approved tools, name who owns billing,
            require account security, and pause staff before API keys or client-file handoffs.
          </p>
          <div class="choice-guide" aria-label="Business rollout rules">
            <article>
              <strong>Approve tools first</strong>
              <p>Pick one or two official AI apps the team may use, and write down what each is for.</p>
            </article>
            <article>
              <strong>Name the owner</strong>
              <p>Choose who controls billing, admin access, recovery codes, and employee offboarding.</p>
            </article>
            <article>
              <strong>Protect client work</strong>
              <p>Decide what files, contracts, customer data, and private notes cannot be pasted into AI.</p>
            </article>
          </div>
          <div class="first-prompt-card" aria-label="Business rollout prompt">
            <span>Owner prompt</span>
            <strong>Use AI to draft the team starting policy.</strong>
            <p>{businessRolloutPrompt}</p>
            <button type="button" class="ghost prompt-copy" onclick={() => copyPrompt('business', businessRolloutPrompt)}>
              {copiedPrompt === 'business' ? 'Copied' : 'Copy prompt'}
            </button>
          </div>
          <div class="startup-checklist" aria-label="Business rollout checklist" onchange={saveStartupProgress}>
            <strong>Team park checklist</strong>
            <label>
              <input type="checkbox" bind:checked={businessChecklist.owner} />
              <span>Billing and account owner named</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={businessChecklist.tools} />
              <span>Approved AI tools chosen for the team</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={businessChecklist.security} />
              <span>2FA or passkeys required for every user</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={businessChecklist.dataRules} />
              <span>Client-file and private-data rules written down</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={businessChecklist.noStaffApi} />
              <span>No staff API keys or unattended agents yet</span>
            </label>
          </div>
          {#if businessReadyToPark}
            <div class="startup-parked" aria-label="Business rollout parked">
              <strong>The team has a safe starting line.</strong>
              <p>Let staff use the approved app for normal work first. Bring Buildbelt back when a repeated workflow needs keys, agents, or file handoff.</p>
            </div>
          {/if}
          <div class="startup-hold">
            <strong>Stop point</strong>
            <p>
              Do not create shared API keys, connect client folders, or let agents run
              until the team has one proven workflow and an owner watching cost and access.
            </p>
          </div>
          <div class="startup-actions">
            <button type="button" class="ghost" onclick={() => (startupStep = 'chooser')}>Back</button>
            <button type="button" class="ghost" onclick={showFullGuide}>Show full business guide</button>
            <button type="button" class="primary" onclick={finishStartup}>Done with startup</button>
          </div>
        </section>
      {:else if startupStep === 'oldComputer'}
        <section class="startup-focus" aria-label="Old computer startup">
          <span>Use what you have</span>
          <h3>Make the old computer your AI practice machine.</h3>
          <p>
            Start with browser tools and account safety. Use the machine to learn what
            work AI actually improves before buying anything new.
          </p>
          <div class="choice-guide" aria-label="Old computer readiness">
            <article>
              <strong>Browser first</strong>
              <p>Use ChatGPT, Claude, or Gemini in the browser before installing local tools or agents.</p>
            </article>
            <article>
              <strong>Make it safe</strong>
              <p>Update the OS and browser, use a password manager, turn on 2FA, and save recovery codes.</p>
            </article>
            <article>
              <strong>Upgrade later</strong>
              <p>Buy only when speed, memory, storage, privacy, or repeated automation becomes the blocker.</p>
            </article>
          </div>
          <div class="first-prompt-card" aria-label="Old computer prompt">
            <span>Computer check prompt</span>
            <strong>Use AI to decide whether this computer is enough.</strong>
            <p>{oldComputerPrompt}</p>
            <button type="button" class="ghost prompt-copy" onclick={() => copyPrompt('old-computer', oldComputerPrompt)}>
              {copiedPrompt === 'old-computer' ? 'Copied' : 'Copy prompt'}
            </button>
          </div>
          <div class="startup-checklist" aria-label="Old computer checklist" onchange={saveStartupProgress}>
            <strong>Old computer park checklist</strong>
            <label>
              <input type="checkbox" bind:checked={oldComputerChecklist.updated} />
              <span>Operating system and browser updated as far as practical</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={oldComputerChecklist.browser} />
              <span>One official AI app opened in the browser</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={oldComputerChecklist.security} />
              <span>Password manager, 2FA, and recovery codes handled</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={oldComputerChecklist.storage} />
              <span>Enough storage cleared for normal browsing and downloads</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={oldComputerChecklist.subscription} />
              <span>Free or personal subscription tested before buying hardware</span>
            </label>
          </div>
          {#if oldComputerReadyToPark}
            <div class="startup-parked" aria-label="Old computer parked">
              <strong>This computer is ready for a learning week.</strong>
              <p>Use browser AI for normal work first. Buy hardware only after a real workflow proves the old machine is the blocker.</p>
            </div>
          {/if}
          <div class="startup-hold">
            <strong>Stop point</strong>
            <p>
              Do not install local models, run agents, or open API billing on this machine
              until browser AI has proved what work you actually want to do.
            </p>
          </div>
          <div class="startup-actions">
            <button type="button" class="ghost" onclick={() => (startupStep = 'chooser')}>Back</button>
            <button type="button" class="ghost" onclick={showFullGuide}>Show details</button>
            <button type="button" class="primary" onclick={finishStartup}>Done with startup</button>
          </div>
        </section>
      {:else}
        <section class="startup-focus" aria-label="AI system buying startup">
          <span>Buying for AI</span>
          <h3>Buy for the workload, not the marketing.</h3>
          <p>
            Decide what you need AI to do before choosing a system. Most people should
            prove the work in a browser app before buying hardware.
          </p>
          <div class="choice-guide" aria-label="AI system buying rules">
            <article>
              <strong>Browser AI is enough</strong>
              <p>Writing, research, planning, documents, and light image work usually do not require a new AI computer.</p>
            </article>
            <article>
              <strong>Buy normal strength</strong>
              <p>If buying anyway, prioritize RAM, storage, comfort, warranty, backup, and support over vague AI branding.</p>
            </article>
            <article>
              <strong>Workstation later</strong>
              <p>Local models, coding agents, video work, and private automation need a clearer spec and budget.</p>
            </article>
          </div>
          <div class="first-prompt-card" aria-label="AI system buying prompt">
            <span>Buying prompt</span>
            <strong>Use AI to decide what kind of computer fits the work.</strong>
            <p>{buyingSystemPrompt}</p>
            <button type="button" class="ghost prompt-copy" onclick={() => copyPrompt('buying-system', buyingSystemPrompt)}>
              {copiedPrompt === 'buying-system' ? 'Copied' : 'Copy prompt'}
            </button>
          </div>
          <div class="startup-checklist" aria-label="AI system buying checklist" onchange={saveStartupProgress}>
            <strong>Buying system park checklist</strong>
            <label>
              <input type="checkbox" bind:checked={buyingSystemChecklist.workload} />
              <span>Main AI workload written down</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={buyingSystemChecklist.browserFirst} />
              <span>Browser AI tested before hardware spending</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={buyingSystemChecklist.security} />
              <span>Account security and billing owner handled first</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={buyingSystemChecklist.specs} />
              <span>RAM, storage, warranty, backup, and support needs written down</span>
            </label>
            <label>
              <input type="checkbox" bind:checked={buyingSystemChecklist.noApi} />
              <span>No API keys, agents, or local model installs before the workflow is real</span>
            </label>
          </div>
          {#if buyingSystemReadyToPark}
            <div class="startup-parked" aria-label="AI system buying parked">
              <strong>You have a buying plan, not a shopping impulse.</strong>
              <p>Use the checklist to shop calmly. If browser AI is still enough, wait and let the workflow prove the purchase.</p>
            </div>
          {/if}
          <div class="startup-hold">
            <strong>Stop point</strong>
            <p>
              Do not buy an AI workstation until the user can name the workload,
              the privacy need, the speed blocker, or the repeated automation it will run.
            </p>
          </div>
          <div class="startup-actions">
            <button type="button" class="ghost" onclick={() => (startupStep = 'chooser')}>Back</button>
            <button type="button" class="ghost" onclick={showFullGuide}>Show details</button>
            <button type="button" class="primary" onclick={finishStartup}>Done with startup</button>
          </div>
        </section>
      {/if}
    {:else}
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
        <button type="button" class="ghost" onclick={() => (startupStep = 'chooser')}>Startup</button>
        <button type="button" class="ghost" onclick={resetStartupProgress}>Reset startup</button>
        <button type="button" class="ghost" onclick={onClose}>Close</button>
        <button type="button" class="primary" onclick={openDoctor}>Run Safe Share Doctor</button>
      </div>
    </footer>
    {/if}
  </div>
</div>
