<script>
  import { reveal } from "$lib/actions/reveal.js";
  import readmeMd from "../../../../README.md?raw";

  const colors = [
    "var(--c-accent)",
    "var(--c-cyan)",
    "var(--c-orange)",
    "var(--c-purple)",
    "var(--c-blue)",
  ];

  function parseCliFlags(md) {
    // Extract the help code block from README
    const helpMatch = md.match(
      /```\n> dtop --help\n([\s\S]*?)```/,
    );
    if (!helpMatch) return [];

    const helpText = helpMatch[1];

    // Extract only the Options section (skip -h/--help and -V/--version)
    const optionsStart = helpText.indexOf("Options:");
    if (optionsStart === -1) return [];
    const optionsBlock = helpText.slice(optionsStart + "Options:".length);

    // Split into individual flag blocks by detecting lines that start with optional spaces then a dash
    const flagBlocks = [];
    const lines = optionsBlock.split("\n");
    let current = null;

    for (const line of lines) {
      // Detect a new flag line: "  -X, --long-name" pattern
      const flagMatch = line.match(
        /^\s{1,4}(-\w),\s+(--[\w-]+)(?:\s+<(\w+)>)?/,
      );
      if (flagMatch) {
        if (current) flagBlocks.push(current);
        current = {
          short: flagMatch[1],
          long: flagMatch[2],
          arg: flagMatch[3] || null,
          descLines: [],
        };
      } else if (current) {
        current.descLines.push(line);
      }
    }
    if (current) flagBlocks.push(current);

    // Filter out -h/--help and -V/--version
    const filtered = flagBlocks.filter(
      (f) => f.long !== "--help" && f.long !== "--version",
    );

    // Parse description and examples from each flag block
    return filtered.map((flag, i) => {
      const rawDesc = flag.descLines
        .map((l) => l.replace(/^\s{10}/, ""))
        .join("\n")
        .trim();

      // Extract first paragraph as description
      const firstPara = rawDesc.split("\n\n")[0].replace(/\n/g, " ").trim();

      // Extract examples - try parenthesized format: "--flag value  (Note)"
      const examples = [];
      let m;
      const parenRegex = /^\s*(--\S+(?:\s+\S+)*?)\s{2,}\(([^)]+)\)/gm;
      while ((m = parenRegex.exec(rawDesc)) !== null) {
        examples.push({ code: m[1].trim(), note: m[2].trim() });
      }

      // Try "value  - description" format (for --icons, --sort style)
      if (examples.length === 0) {
        const dashRegex = /^\s{2,}(\S+)\s+-\s+(.+)/gm;
        while ((m = dashRegex.exec(rawDesc)) !== null) {
          examples.push({
            code: `${flag.long} ${m[1]}`,
            note: m[2].trim(),
          });
        }
      }

      // Try bare example lines: "--flag value" (for --filter style)
      if (examples.length === 0) {
        const bareRegex = /^\s{2,}(--\S+(?:\s*=\S+)?(?:\s+\S+=\S+)*)$/gm;
        while ((m = bareRegex.exec(rawDesc)) !== null) {
          examples.push({ code: m[1].trim(), note: "" });
        }
      }

      return {
        short: flag.short,
        long: flag.long,
        arg: flag.arg,
        color: colors[i % colors.length],
        description: firstPara,
        examples,
      };
    });
  }

  const flags = parseCliFlags(readmeMd);
</script>

<section id="cli" class="relative z-1 mx-auto max-w-300 px-6 pb-24">
  <div use:reveal class="mb-12 text-center">
    <h2
      class="mb-3 font-display text-[clamp(1.8rem,3vw,2.5rem)] font-extrabold tracking-tight text-(--c-text)"
    >
      Command Line
    </h2>
    <p class="text-(--c-text-muted)">
      All the flags you need to get started
    </p>
  </div>

  <div use:reveal={{ delay: 100 }} class="mx-auto grid max-w-180 gap-4">
    {#each flags as flag}
      <div
        class="overflow-hidden border border-(--c-border) bg-(--c-bg-card) transition-colors hover:border-(--c-border-bright)"
      >
        <!-- Flag header -->
        <div
          class="flex items-center justify-between border-b border-(--c-border) px-5 py-3"
        >
          <div class="flex items-center gap-2.5">
            <div
              class="size-2 rounded-full"
              style="background: {flag.color}"
            ></div>
            <code class="font-mono text-sm font-semibold text-(--c-text)">
              {flag.short}, {flag.long}{#if flag.arg}{" "}<span class="text-(--c-text-dim)">&lt;{flag.arg}&gt;</span>{/if}
            </code>
          </div>
        </div>

        <!-- Description and examples -->
        <div class="px-5 py-4">
          <p class="mb-3 text-sm text-(--c-text-muted)">{flag.description}</p>
          {#if flag.examples.length > 0}
            <div class="space-y-1.5">
              {#each flag.examples as example}
                <div class="flex items-center gap-3">
                  <code
                    class="shrink-0 font-mono text-xs"
                    style="color: {flag.color}">{example.code}</code
                  >
                  <span class="text-xs text-(--c-text-dim)"
                    >{example.note}</span
                  >
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <div class="mt-10 text-center">
    <p class="font-mono text-xs text-(--c-text-dim)">
      <span class="text-(--c-accent)">tip</span> &mdash; Combine multiple hosts
      and filters:
      <code
        class="border border-(--c-border-bright) bg-(--c-surface) px-1.5 py-0.5 text-(--c-text)"
        >dtop --host local --host ssh://user@server -f status=running</code
      >
    </p>
  </div>
</section>
