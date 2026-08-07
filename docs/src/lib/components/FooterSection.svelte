<script>
  const links = [
    { href: "https://github.com/amir20/dtop", label: "GitHub" },
    { href: "/changelog", label: "Changelog" },
    { href: "https://github.com/amir20/dtop/issues", label: "Issues" },
  ];
</script>

<footer class="border-t border-(--c-line)">
  <div
    class="mx-auto flex max-w-270 flex-wrap items-center justify-between gap-4 px-4 py-8 md:px-6"
  >
    <p class="text-[0.8125rem] text-(--c-text-dim)">
      dtop — built by
      <a
        href="https://amirraminfar.me/"
        class="transition-colors hover:text-(--c-text-muted)"
      >
        Amir Raminfar
      </a>
    </p>
    <div class="flex gap-6">
      {#each links as link}
        <a
          href={link.href}
          class="text-[0.8125rem] text-(--c-text-dim) transition-colors hover:text-(--c-text-muted)"
        >
          {link.label}
        </a>
      {/each}
    </div>
  </div>
</footer>
