<script setup lang="ts">
import readme from "../../README.md?raw";

const { copy, copied, isSupported } = useClipboard();
const lastCopiedId = ref<string | null>(null);

interface InstallMethod {
  id: string;
  label: string;
  color: string;
  command: string;
}

function parseInstallMethods(md: string): InstallMethod[] {
  const methods: InstallMethod[] = [];
  const config: Record<string, { color: string }> = {
    homebrew: { color: "var(--c-cyan)" },
    docker: { color: "var(--c-purple)" },
    "install script": { color: "var(--c-blue)" },
    "install from source": { color: "var(--c-orange)" },
    nix: { color: "var(--c-cyan)" },
  };

  // Match ### headings under ## Installation, each followed by a ```sh block
  const installSection = md.split("## Installation")[1]?.split(/\n## [^#]/)[0];
  if (!installSection) return [];

  const regex = /### (.+)\n[\s\S]*?```sh\n(.+)\n```/g;
  let match;
  while ((match = regex.exec(installSection)) !== null) {
    const label = match[1].trim();
    const command = match[2].trim();
    const id = label.toLowerCase().replace(/\s+/g, "-");
    const cfg = config[label.toLowerCase()] || { color: "var(--c-text-muted)" };
    methods.push({ id, label, color: cfg.color, command });
  }

  return methods;
}

const installMethods = parseInstallMethods(readme);

function copyCommand(command: string, id: string) {
  copy(command);
  lastCopiedId.value = id;
}

function isCopied(id: string) {
  return copied.value && lastCopiedId.value === id;
}
</script>

<template>
  <section id="install" class="relative z-1 mx-auto max-w-300 px-6 pb-24">
    <div class="mb-12 text-center">
      <h2
        class="mb-3 font-display text-[clamp(1.8rem,3vw,2.5rem)] font-extrabold tracking-tight text-(--c-text)"
      >
        Get Started
      </h2>
      <p class="text-(--c-text-muted)">Choose your preferred installation method</p>
    </div>

    <div class="mx-auto grid max-w-180 gap-3">
      <div
        v-for="method in installMethods"
        :key="method.id"
        class="overflow-hidden border border-(--c-border) bg-(--c-bg-card) transition-colors hover:border-(--c-border-bright)"
      >
        <div class="flex items-center gap-2.5 border-b border-(--c-border) px-5 py-3">
          <div class="size-2 rounded-full" :style="{ background: method.color }" />
          <span class="font-mono text-[0.7rem] font-semibold uppercase tracking-widest text-(--c-text-dim)">
            {{ method.label }}
          </span>
        </div>
        <div class="flex items-center justify-between gap-3 px-5 py-4 font-mono text-sm text-(--c-text)">
          <code class="flex-1 overflow-x-auto whitespace-nowrap">
            <span class="select-none text-(--c-text-dim)">$ </span>{{ method.command }}
          </code>
          <button
            v-if="isSupported"
            class="flex shrink-0 items-center justify-center border border-(--c-border) p-1.5 text-(--c-text-dim) transition-all hover:border-(--c-text-muted) hover:text-(--c-text)"
            aria-label="Copy to clipboard"
            @click="copyCommand(method.command, method.id)"
          >
            <svg v-if="!isCopied(method.id)" class="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
              />
            </svg>
            <svg v-else class="size-4 text-(--c-accent)" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
          </button>
        </div>
      </div>
    </div>

    <div class="mt-12 text-center">
      <p class="mb-4 font-mono text-sm text-(--c-text-dim)">Then run:</p>
      <div
        class="inline-flex items-center gap-3 border border-[rgba(0,255,136,0.2)] bg-(--c-bg-card) px-8 py-4 font-mono text-xl font-semibold text-(--c-accent) shadow-[0_0_40px_-10px_var(--c-accent-dim)]"
      >
        <span class="text-(--c-text-dim)">$</span> dtop<span class="animate-blink">_</span>
      </div>
    </div>
  </section>
</template>
