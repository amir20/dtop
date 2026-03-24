<script>
  import NavBar from "$lib/components/NavBar.svelte";
  import ChangelogSection from "$lib/components/ChangelogSection.svelte";
  import FooterSection from "$lib/components/FooterSection.svelte";
</script>

<svelte:head>
  <title>Changelog - dtop</title>
</svelte:head>

<div id="main" class="min-h-screen overflow-x-hidden">
  <div class="fixed inset-0 z-0 pointer-events-none dot-grid"></div>

  <NavBar />
  <div class="pt-16">
    <ChangelogSection />
  </div>
  <FooterSection />
</div>
