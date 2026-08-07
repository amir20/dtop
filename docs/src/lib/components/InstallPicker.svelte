<script>
  import { browser } from "$app/environment";
  import { installMethods, shortLabel } from "$lib/install.js";

  let { compact = false } = $props();

  let selected = $state(0);
  let copied = $state(false);

  const command = $derived(installMethods[selected]?.command ?? "");

  async function copy() {
    if (!browser) return;
    try {
      await navigator.clipboard.writeText(command);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  }

  function select(i) {
    selected = i;
    copied = false;
  }
</script>

<div class="mx-auto flex w-full max-w-130 flex-col items-center gap-3">
  <div
    class="flex w-full items-center gap-2 rounded-panel border border-(--c-line) bg-(--c-surface) py-2 pr-2 pl-3.5"
  >
    <code
      class="flex-1 overflow-x-auto text-left font-mono text-[0.8125rem] whitespace-nowrap text-(--c-text) [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
    >
      <span class="text-(--c-accent) select-none">$&nbsp;</span>{command}
    </code>
    <button
      class="shrink-0 rounded-md border border-(--c-line-2) bg-(--c-surface-2) px-2.5 py-1.5 font-mono text-xs transition-colors"
      class:text-(--c-text-dim)={!copied}
      class:hover:text-(--c-text)={!copied}
      class:text-(--c-accent)={copied}
      class:border-(--c-accent-2)={copied}
      onclick={copy}
    >
      {copied ? "Copied" : "Copy"}
    </button>
  </div>

  {#if !compact}
    <div class="flex flex-wrap justify-center gap-x-5 gap-y-1">
      {#each installMethods as method, i}
        <button
          class="text-[0.8125rem] transition-colors"
          class:text-(--c-text)={selected === i}
          class:text-(--c-text-dim)={selected !== i}
          class:hover:text-(--c-text-muted)={selected !== i}
          aria-pressed={selected === i}
          onclick={() => select(i)}
        >
          {shortLabel(method)}
        </button>
      {/each}
    </div>
  {/if}
</div>
