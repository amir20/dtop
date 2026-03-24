<script>
  import cargoToml from "../../../../Cargo.toml?raw";

  const version = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? "";
</script>

<section
  class="relative z-1 mx-auto max-w-300 px-6 pt-20 pb-10 text-center md:pt-32 md:pb-16"
>
  <div class="animate-fade-up [animation-delay:0.1s] flex items-center justify-center gap-3">
    <span
      class="mb-8 inline-flex items-center gap-2 border border-[rgba(0,255,136,0.2)] bg-(--c-accent-dim) px-4 py-1.5 font-mono text-xs uppercase tracking-widest text-(--c-accent) before:size-1.5 before:rounded-full before:bg-(--c-accent) before:animate-glow before:content-['']"
    >
      Built with Rust
    </span>
    {#if version}
      <span
        class="mb-8 inline-flex items-center border border-[rgba(0,255,136,0.2)] bg-(--c-accent-dim) px-4 py-1.5 font-mono text-xs tracking-widest text-(--c-accent)"
      >
        v{version}
      </span>
    {/if}
  </div>

  <h1
    class="animate-fade-up [animation-delay:0.2s] font-display text-[clamp(2.5rem,6vw,5rem)] font-extrabold leading-none tracking-tight text-(--c-text)"
  >
    Docker Monitoring<br />
    <span
      class="bg-gradient-to-br from-(--c-accent) via-(--c-cyan) to-(--c-blue) bg-clip-text text-transparent"
    >
      In Your Terminal
    </span>
  </h1>

  <p
    class="animate-fade-up [animation-delay:0.35s] mx-auto mb-4 mt-6 max-w-140 text-lg leading-relaxed text-(--c-text-muted)"
  >
    A real-time TUI for monitoring Docker containers across multiple hosts. Track CPU, memory, and
    network metrics — all from your terminal.
  </p>

  <p class="animate-fade-up [animation-delay:0.35s] font-mono text-sm text-(--c-text-dim)">
    <strong class="font-medium text-(--c-orange)">Blazing fast.</strong> Minimal resource usage.
  </p>

  <div
    class="animate-fade-up [animation-delay:0.5s] relative z-1 mx-auto mt-12 max-w-240 overflow-hidden rounded-xl border border-(--c-border-bright) shadow-[0_0_0_1px_rgba(0,0,0,0.3),0_24px_80px_-12px_rgba(0,0,0,0.6),0_0_120px_-40px_var(--c-accent-dim)]"
  >
    <div
      class="flex items-center gap-2 border-b border-(--c-border) bg-(--c-surface) px-4 py-3"
    >
      <div class="size-3 rounded-full bg-[#ff5f57]"></div>
      <div class="size-3 rounded-full bg-[#febc2e]"></div>
      <div class="size-3 rounded-full bg-[#28c840]"></div>
      <span class="flex-1 text-center font-mono text-xs text-(--c-text-dim)">dtop</span>
    </div>
    <div class="bg-(--c-terminal-bg)">
      <!-- svelte-ignore a11y_media_has_caption -->
      <video class="block w-full" src="/demo.mp4" autoplay loop muted playsinline></video>
    </div>
  </div>
</section>
