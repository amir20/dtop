<script>
    import { browser } from "$app/environment";
    import { reveal } from "$lib/actions/reveal.js";
    import readme from "../../../../README.md?raw";

    let copiedId = $state(null);

    const config = {
        homebrew: { color: "var(--c-cyan)" },
        docker: { color: "var(--c-purple)" },
        "install script": { color: "var(--c-blue)" },
        "install from source": { color: "var(--c-orange)" },
        nix: { color: "var(--c-text-dim)" },
    };

    function parseInstallMethods(md) {
        const methods = [];
        const installSection = md
            .split("## Installation")[1]
            ?.split(/\n## [^#]/)[0];
        if (!installSection) return [];

        const regex = /### (.+)\n[\s\S]*?```sh\n(.+)\n```/g;
        let match;
        while ((match = regex.exec(installSection)) !== null) {
            const label = match[1].trim();
            const command = match[2].trim();
            const id = label.toLowerCase().replace(/\s+/g, "-");
            const cfg = config[label.toLowerCase()] || {
                color: "var(--c-text-muted)",
            };
            methods.push({ id, label, color: cfg.color, command });
        }
        return methods;
    }

    const installMethods = parseInstallMethods(readme);

    async function copyCommand(command, id) {
        if (!browser) return;
        try {
            await navigator.clipboard.writeText(command);
            copiedId = id;
            setTimeout(() => (copiedId = null), 2000);
        } catch (err) {
            console.error("Failed to copy:", err);
        }
    }
</script>

<section id="install" class="relative z-1 mx-auto max-w-300 px-6 pb-24">
    <header
        use:reveal
        class="mb-12 grid grid-cols-12 items-end gap-x-4 border-b border-(--c-border-bright) pb-6 md:mb-16 md:gap-x-6"
    >
        <div class="col-span-12 md:col-span-2">
            <span
                class="font-mono text-[0.7rem] uppercase tracking-[0.22em] text-(--c-accent)"
            >
                § 02 / Install
            </span>
        </div>
        <h2
            class="col-span-12 font-display text-[clamp(2rem,5vw,4rem)] font-extrabold leading-[0.9] tracking-tight text-(--c-text) md:col-span-7"
        >
            Pick a package<br />
            <span class="italic text-(--c-accent)">manager.</span>
        </h2>
        <p
            class="col-span-12 text-sm leading-relaxed text-(--c-text-muted) md:col-span-3"
        >
            Homebrew, Docker, Cargo, Nix, or the install script. One line. Same binary. Pick whatever you already have.
        </p>
    </header>

    <div use:reveal={{ delay: 150 }} class="mx-auto grid max-w-180 gap-3">
        {#each installMethods as method}
            <div
                class="overflow-hidden border border-(--c-border) bg-(--c-bg-card) transition-colors hover:border-(--c-border-bright)"
            >
                <div
                    class="flex items-center gap-2.5 border-b border-(--c-border) px-5 py-3"
                >
                    <div
                        class="size-2 rounded-full"
                        style="background: {method.color}"
                    ></div>
                    <span
                        class="font-mono text-[0.7rem] font-semibold uppercase tracking-widest text-(--c-text-dim)"
                    >
                        {method.label}
                    </span>
                </div>
                <div
                    class="flex items-center justify-between gap-3 px-5 py-4 font-mono text-sm text-(--c-text)"
                >
                    <code class="flex-1 overflow-x-auto whitespace-nowrap">
                        <span class="select-none text-(--c-text-dim)"
                            >$&nbsp;</span
                        >{method.command}
                    </code>
                    <button
                        class="flex shrink-0 items-center justify-center border border-(--c-border) p-1.5 text-(--c-text-dim) transition-all hover:border-(--c-text-muted) hover:text-(--c-text)"
                        aria-label="Copy to clipboard"
                        onclick={() => copyCommand(method.command, method.id)}
                    >
                        {#if copiedId === method.id}
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

    <div class="mt-12 flex items-center justify-center gap-4">
        <span class="font-mono text-sm text-(--c-text-dim)">then run</span>
        <span aria-hidden="true" class="h-px flex-1 max-w-12 bg-(--c-border-bright)"></span>
        <code
            class="inline-flex items-center gap-2 border border-(--c-border-bright) bg-(--c-bg-card) px-6 py-3 font-mono text-lg font-semibold text-(--c-accent)"
        >
            <span class="text-(--c-text-dim)">$</span>dtop<span
                class="animate-blink">_</span
            >
        </code>
    </div>
</section>
