<script setup lang="ts">
import featuresMd from "~/content/features.md?raw";

interface Feature {
  title: string;
  description: string;
}

function parseFeatures(md: string): Feature[] {
  const regex = /- \*\*(.+?)\*\* - (.+)/g;
  const features: Feature[] = [];
  let match;
  while ((match = regex.exec(md)) !== null) {
    features.push({ title: match[1], description: match[2] });
  }
  return features;
}

const features = parseFeatures(featuresMd);

const accentColors = [
  "var(--c-accent)",
  "var(--c-blue)",
  "var(--c-orange)",
];

const cardAccents = [
  "before:bg-(--c-accent)",
  "before:bg-(--c-blue)",
  "before:bg-(--c-orange)",
];

const iconColors = [
  { text: "text-(--c-accent)", bg: "bg-(--c-accent-dim)", border: "border-[rgba(0,255,136,0.2)]" },
  { text: "text-(--c-blue)", bg: "bg-(--c-blue-dim)", border: "border-[rgba(56,189,248,0.2)]" },
  { text: "text-(--c-orange)", bg: "bg-(--c-orange-dim)", border: "border-[rgba(251,146,60,0.2)]" },
];
</script>

<template>
  <section class="relative z-1 mx-auto max-w-300 px-6 py-24">
    <div
      class="animate-fade-up [animation-delay:0.65s] grid grid-cols-1 gap-px border border-(--c-border) bg-(--c-border) md:grid-cols-3"
    >
      <div
        v-for="(feature, i) in features"
        :key="feature.title"
        class="relative bg-(--c-bg) p-10 transition-colors hover:bg-(--c-bg-elevated) before:absolute before:top-0 before:left-0 before:h-0 before:w-0.75 before:transition-all hover:before:h-full"
        :class="cardAccents[i]"
      >
        <div
          class="mb-5 inline-flex size-10 items-center justify-center border font-mono text-xs font-semibold tracking-wide"
          :class="[iconColors[i].text, iconColors[i].bg, iconColors[i].border]"
        >
          {{ String(i + 1).padStart(2, "0") }}
        </div>
        <h3 class="mb-2.5 font-display text-lg font-bold tracking-tight text-(--c-text)">
          {{ feature.title }}
        </h3>
        <p class="text-sm leading-relaxed text-(--c-text-muted)">
          {{ feature.description }}
        </p>
      </div>
    </div>
  </section>
</template>
