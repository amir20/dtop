<script setup lang="ts">
import MarkdownIt from "markdown-it";
import usageMd from "~/content/usage.md?raw";

const md = new MarkdownIt();
const usageHtml = md.render(usageMd);
</script>

<template>
  <section class="relative z-1 mx-auto max-w-180 px-6 pb-24">
    <div class="border border-(--c-border) bg-(--c-bg-card) p-8 md:p-10">
      <h3 class="mb-7 font-display text-xl font-bold tracking-tight text-(--c-text)">
        Keyboard Shortcuts
      </h3>
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div class="usage-content" v-html="usageHtml" />
    </div>
  </section>
</template>

<style>
.usage-content ul {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 0;
}

.usage-content li {
  display: flex;
  align-items: flex-start;
  gap: 0.875rem;
  font-size: 0.9rem;
  line-height: 1.6;
  color: var(--c-text-muted);
}

.usage-content li::before {
  content: "->";
  color: var(--c-accent);
  font-family: "JetBrains Mono", monospace;
  font-size: 0.85rem;
  flex-shrink: 0;
  margin-top: 2px;
}

.usage-content code {
  font-family: "JetBrains Mono", monospace;
  font-size: 0.8rem;
  color: var(--c-text);
  background: var(--c-surface);
  border: 1px solid var(--c-border-bright);
  padding: 2px 8px;
  margin: 0 2px;
}
</style>
