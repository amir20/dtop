<script>
  import NavBar from "$lib/components/NavBar.svelte";
  import HeroSection from "$lib/components/HeroSection.svelte";
  import FeaturesSection from "$lib/components/FeaturesSection.svelte";
  import InstallSection from "$lib/components/InstallSection.svelte";
  import ConfigSection from "$lib/components/ConfigSection.svelte";
  import UsageSection from "$lib/components/UsageSection.svelte";
  import FooterSection from "$lib/components/FooterSection.svelte";
</script>

<svelte:head>
  <title>dtop - Docker Container Monitoring for Your Terminal</title>
</svelte:head>

<div id="main" class="min-h-screen overflow-x-hidden">
  <div class="fixed inset-0 z-0 pointer-events-none dot-grid"></div>

  <NavBar />
  <HeroSection />
  <FeaturesSection />
  <InstallSection />
  <ConfigSection />
  <UsageSection />
  <FooterSection />
</div>
