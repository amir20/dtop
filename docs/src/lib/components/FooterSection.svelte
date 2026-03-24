<script>
  const links = [
    { href: "https://github.com/amir20/dtop", label: "GitHub" },
    { href: "/changelog", label: "Changelog" },
    { href: "https://github.com/amir20/dtop/issues", label: "Issues" },
  ];
</script>

<footer class="relative z-1">
  <div class="h-px bg-gradient-to-r from-transparent via-(--c-accent)/30 to-transparent"></div>
  <div class="border-t border-(--c-border)">
    <div
      class="mx-auto flex max-w-300 flex-col items-center justify-between gap-6 px-6 py-10 sm:flex-row sm:gap-4"
    >
      <div class="flex items-center gap-3">
        <span class="font-mono text-sm font-bold tracking-tight text-(--c-text-dim)">
          <span class="text-(--c-accent)">$</span> dtop
        </span>
        <span class="text-(--c-border-bright)">&middot;</span>
        <span class="text-xs text-(--c-text-dim)">
          Built by
          <a
            href="https://amirraminfar.me/"
            class="text-(--c-text-muted) no-underline transition-colors hover:text-(--c-accent)"
          >
            Amir Raminfar
          </a>
        </span>
      </div>
      <div class="flex gap-6">
        {#each links as link}
          <a
            href={link.href}
            class="font-mono text-xs tracking-wide text-(--c-text-dim) no-underline transition-colors hover:text-(--c-text)"
          >
            {link.label}
          </a>
        {/each}
      </div>
    </div>
  </div>
</footer>
