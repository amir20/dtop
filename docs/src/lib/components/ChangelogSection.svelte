<script>
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

  const sectionColors = {
    Features: "var(--c-accent)",
    "Bug Fixes": "var(--c-orange)",
    Documentation: "var(--c-blue)",
    Miscellaneous: "var(--c-purple)",
    Performance: "var(--c-cyan)",
    Refactor: "var(--c-purple)",
  };

  const colorFor = (title) => sectionColors[title] ?? "var(--c-text-dim)";

  function linkIssues(text) {
    return text.replace(
      /\(#(\d+)\)/g,
      '(<a href="https://github.com/amir20/dtop/issues/$1" class="text-(--c-text-muted) underline underline-offset-2 hover:text-(--c-text)">#$1</a>)',
    );
  }
</script>

<section id="changelog" class="mx-auto max-w-270 px-4 py-14 md:px-6 md:py-20">
  <header class="mb-9 flex max-w-[56ch] flex-col gap-2.5">
    <span class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] text-(--c-text-dim) uppercase">
      Releases
    </span>
    <h2 class="text-[clamp(1.4rem,2.4vw,1.85rem)] font-semibold">Changelog</h2>
    <p class="text-(--c-text-muted)">What changed in each release.</p>
  </header>

  <div class="flex flex-col gap-4">
    {#each entries as entry, i}
      <article class="panel overflow-hidden">
        <div class="flex flex-wrap items-center gap-3 border-b border-(--c-line) px-5 py-3">
          <span class="font-mono text-sm font-medium text-(--c-text)">v{entry.version}</span>
          <span class="font-mono text-xs text-(--c-text-dim)">{entry.date}</span>
          {#if i === 0}
            <span
              class="ml-auto rounded-full border border-(--c-accent-2) bg-(--c-accent-soft) px-2.5 py-0.5 font-mono text-[0.6875rem] text-(--c-accent)"
            >
              Latest
            </span>
          {/if}
        </div>

        <div class="grid divide-y divide-(--c-line) md:grid-cols-2 md:divide-x md:divide-y-0">
          {#each entry.sections as section}
            <div class="px-5 py-4">
              <span
                class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] uppercase"
                style="color: {colorFor(section.title)}"
              >
                {section.title}
              </span>
              <ul class="mt-2.5 flex flex-col gap-1">
                {#each section.items as item}
                  <li class="flex gap-2 text-sm leading-relaxed text-(--c-text-muted)">
                    <span class="mt-0.5 shrink-0 text-(--c-text-dim)">&middot;</span>
                    <span>{@html linkIssues(item)}</span>
                  </li>
                {/each}
              </ul>
            </div>
          {/each}
        </div>
      </article>
    {/each}
  </div>
</section>
