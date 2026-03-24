<script>
  import { reveal } from "$lib/actions/reveal.js";
  import changelogMd from "../../../../CHANGELOG.md?raw";

  function parseChangelog(md) {
    const entries = [];
    const versionRegex = /^## \[([^\]]+)\]\s*-\s*(\S+)/gm;
    let match;
    const splits = [];

    while ((match = versionRegex.exec(md)) !== null) {
      splits.push({
        version: match[1],
        date: match[2],
        start: match.index + match[0].length,
      });
    }

    for (let i = 0; i < splits.length; i++) {
      const end =
        i + 1 < splits.length
          ? splits[i + 1].start -
            splits[i + 1].version.length -
            splits[i + 1].date.length -
            10
          : md.length;
      const body = md.slice(splits[i].start, end).trim();

      const sections = [];
      const sectionRegex = /^### (.+)/gm;
      let secMatch;
      const secSplits = [];

      while ((secMatch = sectionRegex.exec(body)) !== null) {
        secSplits.push({
          title: secMatch[1],
          start: secMatch.index + secMatch[0].length,
        });
      }

      for (let j = 0; j < secSplits.length; j++) {
        const secEnd =
          j + 1 < secSplits.length
            ? secSplits[j + 1].start - secSplits[j + 1].title.length - 5
            : body.length;
        const items = body
          .slice(secSplits[j].start, secEnd)
          .trim()
          .split("\n")
          .map((l) => l.replace(/^- /, "").trim())
          .filter((l) => l.length > 0)
          .map((l) => l.replace(/^\*\(([^)]+)\)\*\s*/, "$1: "));

        if (items.length > 0) {
          sections.push({ title: secSplits[j].title, items });
        }
      }

      if (sections.length > 0) {
        entries.push({
          version: splits[i].version,
          date: splits[i].date,
          sections,
        });
      }
    }

    return entries;
  }

  const entries = parseChangelog(changelogMd);

  const sectionConfig = {
    Features: { color: "var(--c-accent)", icon: "+" },
    "Bug Fixes": { color: "var(--c-orange)", icon: "~" },
    Documentation: { color: "var(--c-blue)", icon: "#" },
    Miscellaneous: { color: "var(--c-purple)", icon: "*" },
    Performance: { color: "var(--c-cyan)", icon: ">" },
    Refactor: { color: "var(--c-purple)", icon: "%" },
  };

  function configFor(title) {
    return sectionConfig[title] ?? { color: "var(--c-text-dim)", icon: "-" };
  }
</script>

<section id="changelog" class="relative z-1 mx-auto max-w-300 px-6 pb-24">
  <div use:reveal class="mb-12 text-center">
    <h2
      class="mb-3 font-display text-[clamp(1.8rem,3vw,2.5rem)] font-extrabold tracking-tight text-(--c-text)"
    >
      Changelog
    </h2>
    <p class="text-(--c-text-muted)">What's new in each release</p>
  </div>

  <div use:reveal={{ delay: 100 }} class="mx-auto max-w-280 space-y-4">
    {#each entries as entry, i}
      <div
        class="group overflow-hidden border bg-(--c-bg-card) transition-colors hover:border-(--c-border-bright)"
        class:border-[rgba(0,255,136,0.25)]={i === 0}
        class:border-(--c-border)={i !== 0}
      >
        <!-- Version header bar -->
        <div
          class="flex items-center gap-4 border-b px-6 py-3"
          class:border-[rgba(0,255,136,0.15)]={i === 0}
          class:border-(--c-border)={i !== 0}
          class:bg-[rgba(0,255,136,0.03)]={i === 0}
        >
          <span class="font-mono text-sm text-(--c-text-dim) select-none"
            >{String(i + 1).padStart(2, "0")}</span
          >
          <span class="text-(--c-border-bright) select-none">&vert;</span>
          <span
            class="font-mono text-base font-bold"
            class:text-(--c-accent)={i === 0}
            class:text-(--c-text)={i !== 0}
          >
            v{entry.version}
          </span>
          <span class="font-mono text-xs text-(--c-text-dim)">{entry.date}</span
          >
          {#if i === 0}
            <span
              class="ml-auto border border-[rgba(0,255,136,0.2)] bg-(--c-accent-dim) px-2.5 py-0.5 font-mono text-[0.6rem] uppercase tracking-widest text-(--c-accent)"
            >
              Latest
            </span>
          {/if}
        </div>

        <!-- Content -->
        <div
          class="grid gap-0 divide-y divide-(--c-border)"
          class:md:grid-cols-2={entry.sections.length > 1}
          class:md:divide-x={entry.sections.length > 1}
          class:md:divide-y-0={entry.sections.length > 1}
        >
          {#each entry.sections as section}
            {@const cfg = configFor(section.title)}
            <div class="px-6 py-4">
              <div class="mb-2.5 flex items-center gap-2">
                <span
                  class="flex size-5 items-center justify-center font-mono text-xs font-bold"
                  style="color: {cfg.color}; background: color-mix(in srgb, {cfg.color} 12%, transparent)"
                  >{cfg.icon}</span
                >
                <span
                  class="font-mono text-[0.7rem] font-semibold uppercase tracking-widest"
                  style="color: {cfg.color}"
                >
                  {section.title}
                </span>
              </div>
              <ul class="space-y-1">
                {#each section.items as item}
                  <li
                    class="flex gap-2 text-sm leading-relaxed text-(--c-text-muted)"
                  >
                    <span class="mt-0.5 shrink-0 text-(--c-text-dim)"
                      >&rarr;</span
                    >
                    <span>{item}</span>
                  </li>
                {/each}
              </ul>
            </div>
          {/each}
        </div>
      </div>
    {/each}
  </div>
</section>
