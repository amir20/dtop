<script>
  const links = [
    { href: "https://github.com/amir20/dtop", label: "GitHub" },
    { href: "https://github.com/amir20/dtop/releases", label: "Releases" },
    { href: "https://github.com/amir20/dtop/issues", label: "Issues" },
  ];
</script>

<footer class="relative z-1 border-t border-(--c-border)">
  <div
    class="mx-auto flex max-w-300 flex-wrap items-center justify-between gap-4 px-6 py-8"
  >
    <div class="text-xs text-(--c-text-dim)">
      Built by
      <a
        href="https://amirraminfar.me/"
        class="text-(--c-text-muted) no-underline transition-colors hover:text-(--c-text)"
      >
        Amir Raminfar
      </a>
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
</footer>
