<script>
  import { reveal } from "$lib/actions/reveal.js";

  const shortcuts = [
    { keys: ["?"], action: "Show help overlay" },
    { keys: ["s"], action: "Open sort selector" },
    { keys: ["c"], action: "Column visibility" },
    { keys: ["Enter"], action: "Open container action menu" },
    { keys: ["\u2192"], action: "View container logs" },
    { keys: ["\u2190"], action: "Return to container list" },
    { keys: ["/"], action: "Search and filter containers" },
    { keys: ["a"], action: "Toggle showing all containers" },
  ];
</script>

<section class="relative z-1 mx-auto max-w-180 px-6 pb-24">
  <div use:reveal class="border border-(--c-border) bg-(--c-bg-card)">
    <div class="border-b border-(--c-border) px-8 py-6 md:px-10">
      <h3 class="font-display text-xl font-bold tracking-tight text-(--c-text)">
        Keyboard Shortcuts
      </h3>
      <p class="mt-1.5 text-sm text-(--c-text-dim)">Navigate everything from the keyboard</p>
    </div>
    <div class="grid grid-cols-1 md:grid-cols-2">
      {#each shortcuts as shortcut, i}
        <div
          class="flex items-center gap-5 border-b border-(--c-border) px-8 py-4 transition-colors hover:bg-(--c-bg-elevated) md:px-10 {i % 2 === 0 ? 'md:border-r' : ''}"
        >
          <div class="flex shrink-0 items-center gap-1.5">
            {#each shortcut.keys as key}
              <kbd
                class="inline-flex min-w-8 items-center justify-center border border-(--c-border-bright) bg-(--c-surface) px-2.5 py-1.5 font-mono text-xs font-medium text-(--c-text) shadow-[0_2px_0_var(--c-border)]"
              >
                {key}
              </kbd>
            {/each}
          </div>
          <span class="text-sm text-(--c-text-muted)">{shortcut.action}</span>
        </div>
      {/each}
    </div>
    <div class="px-8 py-5 md:px-10">
      <p class="font-mono text-xs text-(--c-text-dim)">
        <span class="text-(--c-accent)">tip</span> &mdash; Connect to remote hosts with
        <code class="border border-(--c-border-bright) bg-(--c-surface) px-1.5 py-0.5 text-(--c-text)">dtop --host ssh://user@host</code>
      </p>
    </div>
  </div>
</section>
