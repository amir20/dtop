<script>
  import { reveal } from "$lib/actions/reveal.js";
  import featuresMd from "$lib/content/features.md?raw";

  const regex = /- \*\*(.+?)\*\* - (.+)/g;
  const features = [];
  let match;
  while ((match = regex.exec(featuresMd)) !== null) {
    features.push({ title: match[1], description: match[2] });
  }

  const featureIcons = [
    // Multi-host: network/globe
    `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>`,
    // Metrics: bar chart
    `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M18 20V10"/><path d="M12 20V4"/><path d="M6 20v-6"/></svg>`,
    // Fast: lightning bolt
    `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/></svg>`,
  ];

  const iconColors = [
    {
      text: "text-(--c-accent)",
      bg: "bg-(--c-accent-dim)",
      border: "border-[rgba(0,255,136,0.2)]",
      glow: "group-hover:shadow-[0_0_24px_-4px_var(--c-accent-dim)]",
    },
    {
      text: "text-(--c-blue)",
      bg: "bg-(--c-blue-dim)",
      border: "border-[rgba(56,189,248,0.2)]",
      glow: "group-hover:shadow-[0_0_24px_-4px_var(--c-blue-dim)]",
    },
    {
      text: "text-(--c-orange)",
      bg: "bg-(--c-orange-dim)",
      border: "border-[rgba(251,146,60,0.2)]",
      glow: "group-hover:shadow-[0_0_24px_-4px_var(--c-orange-dim)]",
    },
  ];

  const cardAccents = [
    "before:bg-(--c-accent)",
    "before:bg-(--c-blue)",
    "before:bg-(--c-orange)",
  ];
</script>

<section class="relative z-1 mx-auto max-w-300 px-6 py-24">
  <div
    use:reveal
    class="grid grid-cols-1 gap-px border border-(--c-border) bg-(--c-border) md:grid-cols-3"
  >
    {#each features as feature, i}
      <div
        class="group relative bg-(--c-bg) p-10 transition-all duration-300 hover:bg-(--c-bg-elevated) before:absolute before:top-0 before:left-0 before:h-0 before:w-0.75 before:transition-all before:duration-300 hover:before:h-full {cardAccents[
          i
        ]}"
      >
        <div
          class="mb-5 inline-flex size-11 items-center justify-center border transition-shadow duration-300 {iconColors[
            i
          ].text} {iconColors[i].bg} {iconColors[i].border} {iconColors[i]
            .glow}"
        >
          <span class="size-5">
            {@html featureIcons[i]}
          </span>
        </div>
        <h3
          class="mb-2.5 font-display text-lg font-bold tracking-tight text-(--c-text)"
        >
          {feature.title}
        </h3>
        <p class="text-sm leading-relaxed text-(--c-text-muted)">
          {feature.description}
        </p>
      </div>
    {/each}
  </div>
</section>
