<script>
  import { reveal } from "$lib/actions/reveal.js";
  import featuresMd from "$lib/content/features.md?raw";

  const regex = /- \*\*(.+?)\*\* - (.+)/g;
  const features = [];
  let match;
  while ((match = regex.exec(featuresMd)) !== null) {
    features.push({ title: match[1], description: match[2] });
  }

  const featureMeta = [
    { tag: "topology", meta: "ssh · tcp · tls · local" },
    { tag: "telemetry", meta: "ema · α=0.3 · 500ms" },
    { tag: "footprint", meta: "rust · minimal alloc" },
  ];
</script>

<section
  use:reveal
  class="relative z-1 mx-auto max-w-300 px-6 py-24"
>
  <header
    class="mb-12 grid grid-cols-12 items-end gap-x-4 border-b border-(--c-border-bright) pb-6 md:mb-16 md:gap-x-6"
  >
    <div class="col-span-12 md:col-span-2">
      <span
        class="font-mono text-[0.7rem] uppercase tracking-[0.22em] text-(--c-accent)"
      >
        § 01 / Capabilities
      </span>
    </div>
    <h2
      class="col-span-12 font-display text-[clamp(2rem,5vw,4rem)] font-extrabold leading-[0.9] tracking-tight text-(--c-text) md:col-span-7"
    >
      Three things,<br />
      <span class="italic text-(--c-accent)">done well.</span>
    </h2>
    <p
      class="col-span-12 text-sm leading-relaxed text-(--c-text-muted) md:col-span-3"
    >
      No dashboards to configure, no agents to install, no plans to choose. Open the terminal, see the containers, close the terminal.
    </p>
  </header>

  <div>
    {#each features as feature, i}
      <article
        class="grid grid-cols-12 items-start gap-x-4 border-b border-(--c-border) py-12 md:gap-x-6 md:py-16 last:border-b-0"
      >
        <div class="col-span-2 md:col-span-2">
          <span
            class="font-display text-[clamp(3rem,7vw,5.5rem)] font-extrabold leading-none text-(--c-accent)"
          >
            0{i + 1}
          </span>
        </div>

        <div class="col-span-10 md:col-span-7 md:pr-6">
          <div
            class="mb-3 font-mono text-[0.7rem] uppercase tracking-[0.22em] text-(--c-text-dim)"
          >
            {featureMeta[i].tag} / {featureMeta[i].meta}
          </div>
          <h3
            class="mb-4 font-display text-[clamp(1.6rem,2.8vw,2.4rem)] font-bold leading-[0.95] tracking-tight text-(--c-text)"
          >
            {feature.title}.
          </h3>
          <p class="text-base leading-[1.65] text-(--c-text-muted)">
            {feature.description}
          </p>
        </div>
      </article>
    {/each}
  </div>
</section>
