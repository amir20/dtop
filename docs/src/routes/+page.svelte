<script>
  import NavBar from "#lib/components/NavBar.svelte";
  import HeroSection from "#lib/components/HeroSection.svelte";
  import ShowcaseSection from "#lib/components/ShowcaseSection.svelte";
  import ConfigSection from "#lib/components/ConfigSection.svelte";
  import CliSection from "#lib/components/CliSection.svelte";
  import UsageSection from "#lib/components/UsageSection.svelte";
  import InstallSection from "#lib/components/InstallSection.svelte";
  import FooterSection from "#lib/components/FooterSection.svelte";
</script>

<svelte:head>
  <title>dtop - Docker Container Monitoring for Your Terminal</title>
</svelte:head>

<div id="main" class="min-h-screen overflow-x-hidden">
  <NavBar />
  <HeroSection />
  <ShowcaseSection />
  <ConfigSection />
  <CliSection />
  <UsageSection />
  <InstallSection />
  <FooterSection />
</div>
