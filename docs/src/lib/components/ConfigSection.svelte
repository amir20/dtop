<script>
  import { browser } from "$app/environment";
  import { reveal } from "$lib/actions/reveal.js";
  import configExample from "../../../../config.example.yaml?raw";

  let copiedId = $state(null);

  const colors = ["var(--c-purple)", "var(--c-orange)", "var(--c-cyan)", "var(--c-blue)", "var(--c-accent)"];

  // Section markers: top-level comment lines that start a new config section
  const sectionMarkers = [
    { marker: "# == Hosts ==", label: "Hosts", description: "Docker hosts to monitor" },
    { marker: "# == Icons ==", label: "Icons", description: "Icon style for the UI" },
    { marker: "# == All ==", label: "Show All", description: "Show all containers including stopped" },
    { marker: "# == Sort ==", label: "Sort", description: "Default sort field for container list" },
  ];

  function parseConfigExample(raw) {
    const lines = raw.split("\n");

    // Parse locations from header comments (lines like "# 1. ./config.yaml, ...")
    const locations = [];
    const locRegex = /^#\s+(\d+)\.\s+(.+)/;
    for (const line of lines) {
      const m = line.match(locRegex);
      if (m) {
        const paths = m[2].match(/\.\S+/g) || [];
        const parts = paths.map((p) => p.replace(/,\s*$/, "").replace(/\s+or\s+/, ""));
        if (parts.length > 0) {
          locations.push({
            path: parts[0],
            note: parts.length > 1 ? `or ${parts.slice(1).join(", ")}` : "",
          });
        }
      }
    }

    // Find line indices for each section marker
    const sectionIndices = sectionMarkers.map(({ marker }) => {
      const idx = lines.findIndex((l) => l.startsWith(marker));
      return idx;
    });

    // Build examples from sections
    const examples = sectionMarkers.map((section, i) => {
      const start = sectionIndices[i];
      const end = i + 1 < sectionIndices.length ? sectionIndices[i + 1] : lines.length;
      if (start === -1) return null;

      // Grab all lines for this section, trimming trailing blank lines
      const sectionLines = lines.slice(start, end);
      while (sectionLines.length > 0 && sectionLines[sectionLines.length - 1].trim() === "") {
        sectionLines.pop();
      }

      const code = sectionLines.join("\n");
      const id = section.label.toLowerCase().replace(/[^a-z0-9]+/g, "-");

      return {
        id,
        label: section.label,
        description: section.description,
        color: colors[i % colors.length],
        code,
      };
    }).filter(Boolean);

    return { locations, examples };
  }

  const { locations, examples } = parseConfigExample(configExample);

  function escapeHtml(str) {
    return str.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  function highlightYaml(code) {
    return code
      .split("\n")
      .map((line) => {
        const escaped = escapeHtml(line);

        // Full comment lines
        if (/^\s*#/.test(line)) {
          return `<span class="hl-comment">${escaped}</span>`;
        }

        // Key: value lines
        const kvMatch = escaped.match(/^(\s*-?\s*)([a-zA-Z_][\w-]*)(:\s*)(.*)/);
        if (kvMatch) {
          const [, indent, key, colon, value] = kvMatch;
          let highlightedValue = value;

          if (/^#/.test(value)) {
            // Inline comment after key:
            highlightedValue = `<span class="hl-comment">${value}</span>`;
          } else if (/^(true|false|null|~)$/i.test(value)) {
            highlightedValue = `<span class="hl-bool">${value}</span>`;
          } else if (/^\d[\d.]*$/.test(value)) {
            highlightedValue = `<span class="hl-number">${value}</span>`;
          } else if (value) {
            // Check for trailing inline comment
            const inlineComment = value.match(/^(.+?)\s+(#.*)$/);
            if (inlineComment) {
              highlightedValue = `<span class="hl-string">${inlineComment[1]}</span> <span class="hl-comment">${inlineComment[2]}</span>`;
            } else {
              highlightedValue = `<span class="hl-string">${value}</span>`;
            }
          }

          return `${indent}<span class="hl-key">${key}</span>${colon}${highlightedValue}`;
        }

        // List items with just a value (e.g., "  - status=running")
        const listMatch = escaped.match(/^(\s*-\s+)(.*)/);
        if (listMatch) {
          return `${listMatch[1]}<span class="hl-string">${listMatch[2]}</span>`;
        }

        return escaped;
      })
      .join("\n");
  }

  async function copyCode(code, id) {
    if (!browser) return;
    try {
      await navigator.clipboard.writeText(code);
      copiedId = id;
      setTimeout(() => (copiedId = null), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  }
</script>

<section id="config" class="relative z-1 mx-auto max-w-300 px-6 pb-24">
  <div use:reveal class="mb-12 text-center">
    <h2
      class="mb-3 font-display text-[clamp(1.8rem,3vw,2.5rem)] font-extrabold tracking-tight text-(--c-text)"
    >
      Configuration
    </h2>
    <p class="text-(--c-text-muted)">
      Configure dtop with a YAML file for persistent settings
    </p>
  </div>

  {#if locations.length > 0}
    <div use:reveal={{ delay: 100 }} class="mx-auto mb-10 max-w-180">
      <div class="border border-(--c-border) bg-(--c-bg-card)">
        <div class="border-b border-(--c-border) px-8 py-6 md:px-10">
          <h3
            class="font-display text-xl font-bold tracking-tight text-(--c-text)"
          >
            Config File Locations
          </h3>
          <p class="mt-1.5 text-sm text-(--c-text-dim)">
            Searched in priority order &mdash; first found wins
          </p>
        </div>
        {#each locations as loc, i}
          <div
            class="flex items-center gap-4 border-b border-(--c-border) px-8 py-4 transition-colors hover:bg-(--c-bg-elevated) md:px-10 last:border-b-0"
          >
            <span
              class="flex size-6 shrink-0 items-center justify-center border border-(--c-border-bright) bg-(--c-surface) font-mono text-xs font-medium text-(--c-text-dim)"
              >{i + 1}</span
            >
            <code class="font-mono text-sm text-(--c-text)">{loc.path}</code>
            {#if loc.note}
              <span class="text-sm text-(--c-text-dim)">{loc.note}</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <div use:reveal={{ delay: 200 }} class="mx-auto grid max-w-180 gap-4">
    {#each examples as example}
      <div
        class="overflow-hidden border border-(--c-border) bg-(--c-bg-card) transition-colors hover:border-(--c-border-bright)"
      >
        <div
          class="flex items-center justify-between border-b border-(--c-border) px-5 py-3"
        >
          <div class="flex items-center gap-2.5">
            <div
              class="size-2 rounded-full"
              style="background: {example.color}"
            ></div>
            <span
              class="font-mono text-[0.7rem] font-semibold uppercase tracking-widest text-(--c-text-dim)"
            >
              {example.label}
            </span>
          </div>
          <span class="hidden text-xs text-(--c-text-dim) sm:inline"
            >{example.description}</span
          >
        </div>
        <div class="relative px-5 py-4">
          <pre
            class="yaml-highlight overflow-x-auto font-mono text-sm leading-relaxed text-(--c-text)">{@html highlightYaml(example.code)}</pre>
          <button
            class="absolute right-3 top-3 flex shrink-0 items-center justify-center border border-(--c-border) p-1.5 text-(--c-text-dim) transition-all hover:border-(--c-text-muted) hover:text-(--c-text)"
            aria-label="Copy to clipboard"
            onclick={() => copyCode(example.code, example.id)}
          >
            {#if copiedId === example.id}
              <svg
                class="size-4 text-(--c-accent)"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M5 13l4 4L19 7"
                />
              </svg>
            {:else}
              <svg
                class="size-4"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                />
              </svg>
            {/if}
          </button>
        </div>
      </div>
    {/each}
  </div>

  <div class="mt-10 text-center">
    <p class="font-mono text-xs text-(--c-text-dim)">
      <span class="text-(--c-accent)">tip</span> &mdash; CLI arguments always
      take precedence over config file values
    </p>
  </div>
</section>

<style>
  :global(.yaml-highlight .hl-comment) {
    color: var(--c-text-dim);
    font-style: italic;
  }
  :global(.yaml-highlight .hl-key) {
    color: var(--c-cyan);
  }
  :global(.yaml-highlight .hl-string) {
    color: var(--c-accent);
  }
  :global(.yaml-highlight .hl-bool) {
    color: var(--c-orange);
  }
  :global(.yaml-highlight .hl-number) {
    color: var(--c-orange);
  }
</style>
