<script>
  import { reveal } from "$lib/actions/reveal.js";
  import readmeMd from "../../../../README.md?raw";

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
    return filtered.map((flag) => {
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
        description: firstPara,
        examples,
      };
    });
  }

  const flags = parseCliFlags(readmeMd);
</script>

<section id="cli" class="mx-auto max-w-270 px-4 py-14 md:px-6 md:py-24">
  <header use:reveal class="mb-9 flex max-w-[56ch] flex-col gap-2.5">
    <span class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] text-(--c-text-dim) uppercase">
      Reference
    </span>
    <h2 class="text-[clamp(1.4rem,2.4vw,1.85rem)] font-semibold">Flags</h2>
    <p class="text-(--c-text-muted)">
      Generated from <code class="font-mono text-(--c-text)">dtop --help</code>, so
      it stays correct as the binary changes.
    </p>
  </header>

  <div use:reveal={{ delay: 100 }} class="panel divide-y divide-(--c-line) overflow-hidden">
    {#each flags as flag}
      <div class="grid gap-x-6 gap-y-2 px-5 py-4 md:grid-cols-[14rem_minmax(0,1fr)]">
        <code class="font-mono text-[0.8125rem] font-medium text-(--c-accent)">
          {flag.short}, {flag.long}{#if flag.arg}{" "}<span class="text-(--c-text-dim)">&lt;{flag.arg}&gt;</span>{/if}
        </code>

        <div class="flex flex-col gap-2">
          <p class="text-sm leading-relaxed text-(--c-text-muted)">{flag.description}</p>
          {#if flag.examples.length > 0}
            <div class="flex flex-col gap-1">
              {#each flag.examples as example}
                <div class="flex flex-wrap items-baseline gap-x-3">
                  <code class="font-mono text-xs text-(--c-text)">{example.code}</code>
                  {#if example.note}
                    <span class="text-xs text-(--c-text-dim)">{example.note}</span>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <p class="mt-5 text-[0.8125rem] text-(--c-text-dim)">
    Hosts and filters combine — <code class="font-mono text-(--c-text-muted)"
      >dtop --host local --host ssh://user@server -f status=running</code
    >
  </p>
</section>
