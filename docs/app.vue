<script setup>
const isDark = useDark({
  storageKey: "vueuse-color-scheme",
});
const toggleDark = useToggle(isDark);

useHead({
  title: "dtop - Docker Container Monitoring for Your Terminal",
  htmlAttrs: { lang: "en" },
});
</script>

<template>
  <div class="min-h-screen overflow-x-hidden">
    <!-- Dot grid background -->
    <div
      class="fixed inset-0 z-0 pointer-events-none"
      :style="{
        opacity: 'var(--c-dot-opacity)',
        backgroundImage: 'radial-gradient(circle, var(--c-border) 1px, transparent 1px)',
        backgroundSize: '32px 32px',
        maskImage: 'radial-gradient(ellipse 80% 60% at 50% 30%, black 20%, transparent 70%)',
      }"
    />

    <NavBar :is-dark="isDark" @toggle-theme="toggleDark()" />
    <HeroSection />
    <FeaturesSection />
    <InstallSection />
    <UsageSection />
    <FooterSection />
  </div>
</template>
