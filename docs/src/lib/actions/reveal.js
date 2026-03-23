/**
 * Svelte action: scroll-triggered reveal animation using IntersectionObserver.
 * Usage: <div use:reveal> or <div use:reveal={{ delay: 200 }}>
 */
export function reveal(node, opts = {}) {
  const { delay = 0, threshold = 0.15 } = opts;

  node.classList.add("scroll-hidden");

  const observer = new IntersectionObserver(
    ([entry]) => {
      if (entry.isIntersecting) {
        if (delay) {
          node.style.animationDelay = `${delay}ms`;
        }
        node.classList.remove("scroll-hidden");
        node.classList.add("scroll-visible");
        observer.unobserve(node);
      }
    },
    { threshold },
  );

  observer.observe(node);

  return {
    destroy() {
      observer.disconnect();
    },
  };
}
