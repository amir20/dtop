<script>
  import { reveal } from "$lib/actions/reveal.js";

  const shortcuts = [
    { key: "↑↓", action: "Move between containers" },
    { key: "→", action: "Stream this container's logs" },
    { key: "←", action: "Back to the container list" },
    { key: "Enter", action: "Start, stop, restart, remove" },
    { key: "/", action: "Search by name or ID" },
    { key: "s", action: "Choose the sort field" },
    { key: "c", action: "Show and reorder columns" },
    { key: "a", action: "Include stopped containers" },
  ];
</script>

<section class="mx-auto max-w-270 px-4 py-14 md:px-6 md:py-24">
  <header use:reveal class="mb-9 flex max-w-[56ch] flex-col gap-2.5">
    <span class="font-mono text-[0.6875rem] font-medium tracking-[0.11em] text-(--c-text-dim) uppercase">
      Keys
    </span>
    <h2 class="text-[clamp(1.4rem,2.4vw,1.85rem)] font-semibold">
      Hands stay on the keyboard
    </h2>
    <p class="text-(--c-text-muted)">
      Eight bindings cover the whole surface. Press
      <code class="font-mono text-(--c-text)">?</code> anywhere for the overlay.
    </p>
  </header>

  <div use:reveal={{ delay: 100 }} class="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
    {#each shortcuts as shortcut}
      <div class="flex items-center gap-3.5 rounded-md border border-(--c-line) bg-(--c-surface) px-3.5 py-2.5">
        <kbd
          class="grid h-6.5 min-w-7.5 shrink-0 place-items-center rounded-[5px] border border-(--c-line-2) bg-(--c-surface-2) px-1.5 font-mono text-xs font-medium text-(--c-text)"
        >
          {shortcut.key}
        </kbd>
        <span class="text-[0.8125rem] text-(--c-text-muted)">{shortcut.action}</span>
      </div>
    {/each}
  </div>
</section>
