<script>
  import { browser } from "$app/environment";

  let isDark = $state(true);
  let menuOpen = $state(false);

  if (browser) {
    isDark = document.documentElement.classList.contains("dark");
  }

  function toggleTheme() {
    isDark = !isDark;
    document.documentElement.classList.toggle("dark", isDark);
    localStorage.setItem("theme", isDark ? "dark" : "light");
  }

  const links = [
    { href: "/#install", label: "Install" },
    { href: "/#config", label: "Config" },
    { href: "/#cli", label: "Reference" },
    { href: "/changelog", label: "Changelog" },
  ];
</script>

<nav class="sticky top-0 z-50 border-b border-(--c-line) bg-(--c-bg)">
  <div class="mx-auto flex h-14 max-w-310 items-center gap-6 px-4 md:px-6">
    <a href="/" class="flex items-center gap-2 text-[0.95rem] font-semibold tracking-tight">
      <span class="size-1.5 rounded-full bg-(--c-accent)"></span>
      dtop
    </a>

    {#each links as link}
      <a
        href={link.href}
        class="hidden text-sm text-(--c-text-muted) transition-colors hover:text-(--c-text) md:inline"
      >
        {link.label}
      </a>
    {/each}

    <span class="flex-1"></span>

    <button
      class="flex size-8 items-center justify-center rounded-md border border-(--c-line-2) text-(--c-text-muted) transition-colors hover:border-(--c-text-dim) hover:text-(--c-text)"
      aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
      onclick={toggleTheme}
    >
      {#if isDark}
        <svg class="size-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
          />
        </svg>
      {:else}
        <svg class="size-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
          />
        </svg>
      {/if}
    </button>

    <a
      href="https://github.com/amir20/dtop"
      class="hidden rounded-md border border-(--c-line-2) px-2.5 py-1.5 text-sm text-(--c-text-muted) transition-colors hover:border-(--c-text-dim) hover:text-(--c-text) md:inline-block"
    >
      GitHub
    </a>

    <a
      href="/#install"
      class="rounded-md border border-(--c-text) bg-(--c-text) px-3 py-1.5 text-sm font-medium text-(--c-bg) transition-opacity hover:opacity-85"
    >
      Get dtop
    </a>

    <button
      class="flex size-8 items-center justify-center rounded-md border border-(--c-line-2) text-(--c-text-muted) transition-colors hover:text-(--c-text) md:hidden"
      aria-label="Toggle menu"
      aria-expanded={menuOpen}
      onclick={() => (menuOpen = !menuOpen)}
    >
      <svg class="size-3.5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24">
        {#if menuOpen}
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
        {:else}
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h16M4 12h16M4 18h16" />
        {/if}
      </svg>
    </button>
  </div>

  {#if menuOpen}
    <div class="border-t border-(--c-line) md:hidden">
      <div class="flex flex-col px-4 py-2">
        {#each links as link}
          <a
            href={link.href}
            class="py-2 text-sm text-(--c-text-muted) transition-colors hover:text-(--c-text)"
            onclick={() => (menuOpen = false)}
          >
            {link.label}
          </a>
        {/each}
        <a
          href="https://github.com/amir20/dtop"
          class="py-2 text-sm text-(--c-text-muted) transition-colors hover:text-(--c-text)"
          onclick={() => (menuOpen = false)}
        >
          GitHub
        </a>
      </div>
    </div>
  {/if}
</nav>
