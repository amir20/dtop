<script>
  import { browser } from "$app/environment";
  import { reveal } from "$lib/actions/reveal.js";
  import configExample from "../../../../config.example.yaml?raw";

  let copiedId = $state(null);

  // Section markers: top-level comment lines that start a new config section
  const sectionMarkers = [
    { marker: "# == Hosts ==", label: "Hosts", description: "Docker hosts to monitor" },
    { marker: "# == Icons ==", label: "Icons", description: "Icon style for the UI" },
    { marker: "# == All ==", label: "Show All", description: "Show all containers including stopped" },
    { marker: "# == Sort ==", label: "Sort", description: "Default sort field for container list" },
    { marker: "# == Columns ==", label: "Columns", description: "Column visibility and order" },
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

<section id="config" class="mx-auto max-w-270 px-4 py-14 md:px-6 md:py-24">
  <header use:reveal class="mb-9 flex max-w-[56ch] flex-col gap-2.5">
    <span class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] text-(--c-text-dim) uppercase">
      Config
    </span>
    <h2 class="text-[clamp(1.4rem,2.4vw,1.85rem)] font-semibold">
      Write it down once, or let dtop do it
    </h2>
    <p class="text-(--c-text-muted)">
      Every flag can live in a YAML file. Or press
      <code class="font-mono text-(--c-text)">Ctrl-S</code> in the app and dtop saves
      your columns, sort and filters for you — keys you set by hand are preserved.
    </p>
  </header>

  <div use:reveal={{ delay: 100 }} class="grid items-start gap-4 lg:grid-cols-2">
    {#if locations.length > 0}
      <div class="panel p-6">
        <span class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] text-(--c-text-dim) uppercase">
          Search order
        </span>
        <div class="mt-3.5 flex flex-col gap-2 font-mono text-[0.8125rem] text-(--c-text-muted)">
          {#each locations as loc, i}
            <div class="flex gap-3">
              <span class="text-(--c-text-dim)">{i + 1}</span>
              <span class="text-(--c-text)">{loc.path}</span>
              {#if loc.note}
                <span class="text-(--c-text-dim)">{loc.note}</span>
              {/if}
            </div>
          {/each}
        </div>
        <p class="mt-5 text-[0.8125rem] text-(--c-text-muted)">
          First file found wins. Command-line flags override whatever the file says.
        </p>
      </div>
    {/if}

    {#each examples as example}
      <div class="panel overflow-hidden">
        <div class="flex items-center justify-between gap-3 border-b border-(--c-line) px-4 py-2.5">
          <span class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] text-(--c-text-dim) uppercase">
            {example.label}
          </span>
          <span class="hidden text-xs text-(--c-text-dim) sm:inline">{example.description}</span>
        </div>
        <div class="relative">
          <pre class="yaml-highlight overflow-x-auto px-4 py-3.5 font-mono text-[0.8125rem] leading-[1.8] text-(--c-text-muted)">{@html highlightYaml(example.code)}</pre>
          <button
            class="absolute top-2.5 right-2.5 flex items-center justify-center rounded-md border border-(--c-line-2) bg-(--c-surface-2) p-1.5 transition-colors"
            class:text-(--c-text-dim)={copiedId !== example.id}
            class:hover:text-(--c-text)={copiedId !== example.id}
            class:text-(--c-accent)={copiedId === example.id}
            aria-label="Copy to clipboard"
            onclick={() => copyCode(example.code, example.id)}
          >
            {#if copiedId === example.id}
              <svg class="size-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
              </svg>
            {:else}
              <svg class="size-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                />
              </svg>
            {/if}
          </button>
        </div>
      </div>
    {/each}
  </div>
</section>

<style>
  /* config.example.yaml is mostly comments, so they carry real information
     here and need to stay readable rather than sit at label contrast. */
  :global(.yaml-highlight .hl-comment) {
    color: var(--c-text-muted);
  }
  :global(.yaml-highlight .hl-key) {
    color: var(--c-accent);
  }
  :global(.yaml-highlight .hl-string) {
    color: var(--c-text);
  }
  :global(.yaml-highlight .hl-bool),
  :global(.yaml-highlight .hl-number) {
    color: var(--c-warn);
  }
</style>
