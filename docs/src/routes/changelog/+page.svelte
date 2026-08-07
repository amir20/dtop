<script>
  import NavBar from "$lib/components/NavBar.svelte";
  import ChangelogSection from "$lib/components/ChangelogSection.svelte";
  import FooterSection from "$lib/components/FooterSection.svelte";
</script>

<svelte:head>
  <title>Changelog - dtop</title>
</svelte:head>

<div id="main" class="min-h-screen overflow-x-hidden">
  <NavBar />
  <ChangelogSection />
  <FooterSection />
</div>
