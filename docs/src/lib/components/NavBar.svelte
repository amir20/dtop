<script>
    import { browser } from "$app/environment";

    let isDark = $state(true);

    if (browser) {
        isDark = document.documentElement.classList.contains("dark");
    }

    function toggleTheme() {
        isDark = !isDark;
        document.documentElement.classList.toggle("dark", isDark);
        localStorage.setItem("theme", isDark ? "dark" : "light");
    }
</script>

<nav
    class="sticky top-0 z-50 border-b border-(--c-border) backdrop-blur-xl backdrop-saturate-180 bg-(--c-nav-bg)"
>
    <div class="mx-auto flex h-16 max-w-300 items-center justify-between px-6">
        <div class="flex items-center gap-6">
            <a href="/" class="font-mono text-xl font-bold tracking-tight text-(--c-text) no-underline">
                <span class="text-(--c-accent)">$</span> dtop<span
                    class="animate-blink text-(--c-accent)">_</span
                >
            </a>
            <a
                href="/#config"
                class="font-mono text-[0.8rem] text-(--c-text-muted) no-underline transition-colors hover:text-(--c-text)"
            >
                Config
            </a>
            <a
                href="/changelog"
                class="font-mono text-[0.8rem] text-(--c-text-muted) no-underline transition-colors hover:text-(--c-text)"
            >
                Changelog
            </a>
        </div>
        <div class="flex items-center gap-6">
            <a
                href="https://github.com/amir20/dtop"
                class="flex items-center text-(--c-text-muted) transition-colors hover:text-(--c-text)"
                aria-label="GitHub"
            >
                <svg class="size-5.5" viewBox="0 0 16 16" fill="currentColor">
                    <path
                        d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
                    />
                </svg>
            </a>
            <button
                class="flex size-9 items-center justify-center border border-(--c-border-bright) text-(--c-text-muted) transition-all hover:border-(--c-text-muted) hover:text-(--c-text)"
                aria-label={isDark
                    ? "Switch to light mode"
                    : "Switch to dark mode"}
                onclick={toggleTheme}
            >
                {#if isDark}
                    <svg
                        class="size-4.5"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                    >
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
                        />
                    </svg>
                {:else}
                    <svg
                        class="size-4.5"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                    >
                        <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            stroke-width="2"
                            d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
                        />
                    </svg>
                {/if}
            </button>
            <a
                href="/#install"
                class="bg-(--c-accent) px-5 py-2 font-mono text-[0.8rem] font-medium tracking-wide text-(--c-bg) no-underline transition-all hover:shadow-[0_0_20px_var(--c-accent-glow)] hover:-translate-y-px"
            >
                Install
            </a>
        </div>
    </div>
</nav>
