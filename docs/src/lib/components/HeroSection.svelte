<script>
  import cargoToml from "../../../../Cargo.toml?raw";
  import InstallPicker from "./InstallPicker.svelte";
  import TuiDemo from "./TuiDemo.svelte";

  const version = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? "";
</script>

<section class="mx-auto max-w-270 px-4 pt-14 pb-8 md:px-6 md:pt-24 md:pb-12">
  <div class="mx-auto flex max-w-165 flex-col items-center gap-5 text-center">
    <span
      class="inline-flex items-center gap-2 rounded-full border border-(--c-line) bg-(--c-surface) px-3 py-1 font-mono text-xs text-(--c-text-muted)"
    >
      <span class="size-1.5 rounded-full bg-(--c-accent)"></span>
      {#if version}v{version} &middot; {/if}built with Rust
    </span>

    <h1 class="text-[clamp(2rem,4.6vw,3rem)] font-semibold">
      Monitor containers across every host
    </h1>

    <p class="max-w-[52ch] text-[1.0625rem] leading-relaxed text-(--c-text-muted)">
      dtop streams live CPU, memory, network and disk metrics from local, SSH, TCP
      and TLS Docker daemons into a single table in your terminal.
    </p>

    <div class="mt-1 w-full">
      <InstallPicker />
    </div>
  </div>
</section>

<div class="mx-auto max-w-270 px-4 pb-4 md:px-6">
  <TuiDemo />
</div>
