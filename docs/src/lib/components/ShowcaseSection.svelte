<script>
  import { onMount } from "svelte";
  import { reveal } from "#lib/actions/reveal.js";

  // ---- 1. Every host in one table

  const endpoints = [
    { scheme: "local", target: "/var/run/docker.sock" },
    { scheme: "ssh", target: "deploy@edge-01:2222" },
    { scheme: "tcp", target: "192.168.1.100:2375" },
    { scheme: "tls", target: "ops@db-eu:2376" },
  ];

  // ---- 2. Live stats: a sparkline fed the same way the table is (EMA, 500ms)

  const GLYPHS = "▁▂▃▄▅▆▇█";
  let spark = $state("▂▃▅▆▇█▇▆▄▃▂▁▂▄▆▇█▇▅▄▃▂▁▂▃▅▆▇▆▄▃▂▁");

  onMount(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;

    let phase = 0;
    const timer = setInterval(() => {
      phase += 0.42;
      const value = (Math.sin(phase) + Math.sin(phase * 0.43) + 2) / 4;
      const next = GLYPHS[Math.max(0, Math.min(7, Math.round(value * 7)))];
      spark = spark.slice(1) + next;
    }, 340);

    return () => clearInterval(timer);
  });

  // ---- 3. Bulk actions: marked rows are bold magenta (`UiStyles::marked`)

  const bulkRows = [
    { name: "shop-api", project: "shop", marked: true },
    { name: "shop-worker", project: "shop", marked: true },
    { name: "shop-redis", project: "shop", marked: true },
    { name: "grafana", project: "monitoring", marked: false },
  ];

  // ---- 4. Native terminal support

  const terminalFeatures = [
    {
      title: "Native panes",
      body: "With --panes, Shell and → open in a tmux or Rex pane beside the list, on any host dtop can reach, SSH included.",
      where: "tmux · Rex",
    },
    {
      title: "Tab status",
      body: "The tab is flagged while a bulk action runs, when it finishes or fails, when a host drops, and when a container turns unhealthy.",
      where: "Rex",
    },
    {
      title: "App logos",
      body: "Each container's image logo sits next to its name, drawn by the terminal itself.",
      where: "Rex · Ghostty · kitty",
    },
    {
      title: "Works over SSH",
      body: "Run dtop on a server and it still reaches your local terminal: icons, logos, tab title and status travel through the SSH session.",
      where: "Everywhere",
    },
  ];
</script>

{#snippet eyebrow(text)}
  <span class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] text-(--c-text-dim) uppercase">
    {text}
  </span>
{/snippet}

<section id="features" class="mx-auto max-w-270 px-4 py-14 md:px-6 md:py-24">
  <header use:reveal class="mb-12 flex max-w-[56ch] flex-col gap-2.5">
    {@render eyebrow("What it does well")}
    <h2 class="text-[clamp(1.4rem,2.4vw,1.85rem)] font-semibold">
      Nothing to deploy, nothing to configure
    </h2>
    <p class="text-(--c-text-muted)">
      No agents on the machines you're watching, no collector, no account. Point
      dtop at a socket and it starts drawing.
    </p>
  </header>

  <div class="flex flex-col gap-16 md:gap-24">
    <!-- 1. Hosts -->
    <article use:reveal class="grid items-center gap-6 [&>*]:min-w-0 md:grid-cols-2 md:gap-12">
      <div class="flex flex-col gap-3">
        {@render eyebrow("Multi-host")}
        <h3 class="text-xl font-semibold">Every host in one table</h3>
        <p class="text-(--c-text-muted)">
          Local, SSH, TCP and TLS daemons side by side, sorted together, with a Host
          column when there's more than one. Over SSH, containers share a few pooled
          connections instead of one each, so 30 containers use 4.
        </p>
      </div>
      <div class="panel p-5 font-mono text-[0.8125rem]">
        {#each endpoints as endpoint}
          <div class="flex items-center gap-3 py-1 whitespace-nowrap">
            <span class="min-w-11 text-(--c-accent)">{endpoint.scheme}</span>
            <span class="text-(--c-text-muted)">{endpoint.target}</span>
          </div>
        {/each}
        <div class="mt-4 flex gap-6 border-t border-(--c-line) pt-4">
          <div>
            <div class="font-body text-2xl font-semibold text-(--c-text)">0</div>
            <div class="text-xs text-(--c-text-dim)">agents to deploy</div>
          </div>
          <div>
            <div class="font-body text-2xl font-semibold text-(--c-text)">3.8 MB</div>
            <div class="text-xs text-(--c-text-dim)">single binary</div>
          </div>
        </div>
      </div>
    </article>

    <!-- 2. Stats -->
    <article use:reveal class="grid items-center gap-6 [&>*]:min-w-0 md:grid-cols-2 md:gap-12">
      <div class="flex flex-col gap-3 md:order-2">
        {@render eyebrow("Live stats")}
        <h3 class="text-xl font-semibold">The machine and its containers, together</h3>
        <p class="text-(--c-text-muted)">
          CPU, memory, network and disk per container, smoothed so the numbers settle
          instead of flicker. A header adds the machine itself, local or read over SSH
          from the server's own /proc, next to what its containers add up to.
        </p>
      </div>
      <div class="panel overflow-hidden p-5 font-mono text-[0.8125rem] md:order-1">
        <span class="block overflow-hidden text-[1.4em] tracking-[-0.03em] whitespace-nowrap text-(--c-accent)">
          {spark}
        </span>
        <div class="mt-4 flex flex-col gap-1 border-t border-(--c-line) pt-4 whitespace-nowrap text-(--c-text-muted)">
          <div>
            <span class="font-semibold text-(--c-cyan)">System</span>  nas
            <span class="font-semibold text-(--c-cyan)">CPU</span>
            <span class="text-(--c-ok)">12.7%</span> of 6 cores
          </div>
          <div>
            <span class="font-semibold text-(--c-cyan)">Docker</span>  nas
            <span class="font-semibold text-(--c-cyan)">CPU</span>
            <span class="text-(--c-ok)"> 0.6%</span>  16/16 running
          </div>
          <div class="mt-2 text-xs text-(--c-text-dim)">
            <span class="text-(--c-ok)">&#9632;</span> 0&ndash;50
            <span class="ml-1.5 text-(--c-warn)">&#9632;</span> 50&ndash;80
            <span class="ml-1.5 text-(--c-crit)">&#9632;</span> 80+
          </div>
        </div>
      </div>
    </article>

    <!-- 3. Bulk actions -->
    <article use:reveal class="grid items-center gap-6 [&>*]:min-w-0 md:grid-cols-2 md:gap-12">
      <div class="flex flex-col gap-3">
        {@render eyebrow("Actions")}
        <h3 class="text-xl font-semibold">Act on many containers at once</h3>
        <p class="text-(--c-text-muted)">
          Mark containers with <kbd class="font-mono text-(--c-text)">Space</kbd>, a whole
          compose project with <kbd class="font-mono text-(--c-text)">p</kbd>, or
          everything listed with <kbd class="font-mono text-(--c-text)">Ctrl+A</kbd>,
          then start, stop, restart or remove them together. Remove asks twice, failures
          stay marked for a retry, and one summary tells you how it went.
        </p>
      </div>
      <div class="panel overflow-hidden p-5 font-mono text-[0.8125rem]">
        <div class="truncate text-(--c-text-muted)">7 containers, 3 selected (Enter for actions, Esc to clear)</div>
        <div class="mt-3 flex flex-col gap-0.5">
          {#each bulkRows as row}
            <div
              class="flex gap-4 whitespace-nowrap"
              class:font-semibold={row.marked}
              class:text-(--c-magenta)={row.marked}
              class:text-(--c-text-muted)={!row.marked}
            >
              <span class="w-28">{row.name}</span>
              <span>{row.project}</span>
            </div>
          {/each}
        </div>
        <div class="mt-4 inline-block rounded-md border border-(--c-line-2) bg-(--c-surface-2) px-3 py-1.5 text-(--c-text)">
          Restarted 3 containers
        </div>
      </div>
    </article>

    <!-- 4. Terminal-native, Rex first -->
    <article id="rex" use:reveal class="grid items-start gap-6 [&>*]:min-w-0 md:grid-cols-2 md:gap-12">
      <div class="flex flex-col gap-3 md:sticky md:top-24">
        {@render eyebrow("Native Rex support")}
        <h3 class="text-xl font-semibold">Made for your terminal, not just inside it</h3>
        <p class="text-(--c-text-muted)">
          dtop asks the terminal what it can do and uses it: Nerd Font icons, clickable
          names, a tab title that follows what you're looking at. In
          <a class="text-(--c-text) underline underline-offset-3" href="https://www.superlogical.com/rex"
            >Rex</a
          >
          it goes further, with a tab that tells you when something needs a look. In
          tmux and Rex, shells and logs open in their own panes.
        </p>
        <pre class="panel mt-2 overflow-x-auto px-4 py-3 font-mono text-[0.8125rem] text-(--c-text)">dtop --panes --host ssh://user@nas</pre>
      </div>
      <div class="grid gap-3">
        {#each terminalFeatures as feature}
          <div class="panel flex flex-col gap-1.5 p-5">
            <div class="flex items-baseline justify-between gap-3">
              <h4 class="font-semibold">{feature.title}</h4>
              <span class="shrink-0 font-mono text-[0.6875rem] text-(--c-text-dim)">{feature.where}</span>
            </div>
            <p class="text-sm leading-relaxed text-(--c-text-muted)">{feature.body}</p>
          </div>
        {/each}
      </div>
    </article>
  </div>
</section>
