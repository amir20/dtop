<script>
  import { onMount } from "svelte";
  import { reveal } from "$lib/actions/reveal.js";
  import featuresMd from "$lib/content/features.md?raw";

  const regex = /- \*\*(.+?)\*\* - (.+)/g;
  const features = [];
  let match;
  while ((match = regex.exec(featuresMd)) !== null) {
    features.push({ title: match[1], description: match[2] });
  }

  const endpoints = [
    { scheme: "local", target: "/var/run/docker.sock" },
    { scheme: "ssh", target: "deploy@edge-01:2222" },
    { scheme: "tcp", target: "192.168.1.100:2375" },
    { scheme: "tls", target: "ops@db-eu:2376" },
  ];

  const stats = [
    { value: "3.8 MB", label: "release binary" },
    { value: "1.9 MB", label: "without self-update" },
    { value: "0", label: "agents to deploy" },
  ];

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
</script>

<section class="mx-auto max-w-270 px-4 py-14 md:px-6 md:py-24">
  <header use:reveal class="mb-9 flex max-w-[56ch] flex-col gap-2.5">
    <span class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] text-(--c-text-dim) uppercase">
      Overview
    </span>
    <h2 class="text-[clamp(1.4rem,2.4vw,1.85rem)] font-semibold">
      Nothing to deploy, nothing to configure
    </h2>
    <p class="text-(--c-text-muted)">
      No agents on the machines you're watching, no collector, no account. Point
      dtop at a socket and it starts drawing.
    </p>
  </header>

  <div use:reveal={{ delay: 100 }} class="grid gap-4 md:grid-cols-3">
    {#each features as feature, i}
      <article class="panel flex flex-col gap-2.5 p-6">
        <h3 class="text-base font-semibold">{feature.title}</h3>
        <p class="text-sm leading-relaxed text-(--c-text-muted)">{feature.description}</p>

        <div class="mt-3 border-t border-(--c-line) pt-4 font-mono text-xs text-(--c-text-dim)">
          {#if i === 0}
            {#each endpoints as endpoint}
              <div class="flex items-center gap-2.5 py-0.5 whitespace-nowrap">
                <span class="min-w-11 text-(--c-accent)">{endpoint.scheme}</span>
                <span>{endpoint.target}</span>
              </div>
            {/each}
          {:else if i === 1}
            <span class="block overflow-hidden text-[1.25em] tracking-[-0.03em] whitespace-nowrap text-(--c-accent)">
              {spark}
            </span>
            <div class="mt-2.5 leading-[1.9]">
              <div>ema &alpha; 0.3 &middot; tick 500 ms</div>
              <div>
                <span class="text-(--c-ok)">&#9632;</span> 0&ndash;50
                <span class="ml-1.5 text-(--c-warn)">&#9632;</span> 50&ndash;80
                <span class="ml-1.5 text-(--c-crit)">&#9632;</span> 80+
              </div>
            </div>
          {:else}
            {#each stats as stat}
              <div class="flex items-baseline gap-1.5 py-0.5">
                <span class="font-body text-2xl font-semibold tracking-[-0.03em] text-(--c-text)">
                  {stat.value}
                </span>
                <span>{stat.label}</span>
              </div>
            {/each}
          {/if}
        </div>
      </article>
    {/each}
  </div>
</section>
