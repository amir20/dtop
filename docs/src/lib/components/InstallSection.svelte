<script>
  import { reveal } from "$lib/actions/reveal.js";
  import InstallPicker from "./InstallPicker.svelte";
</script>

<section id="install" class="mx-auto max-w-270 px-4 pt-6 pb-14 md:px-6 md:pb-20">
  <div use:reveal class="panel px-6 py-10 text-center md:px-12 md:py-14">
    <h2 class="text-[clamp(1.35rem,2.4vw,1.75rem)] font-semibold">
      Start watching in one command
    </h2>
    <p class="mx-auto mt-2.5 max-w-[44ch] text-(--c-text-muted)">
      macOS and Linux, Intel and ARM. No account, no telemetry.
    </p>
    <div class="mt-7">
      <InstallPicker />
    </div>
  </div>
</section>
