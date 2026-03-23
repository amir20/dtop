import tailwindcss from "@tailwindcss/vite";

export default defineNuxtConfig({
  compatibilityDate: "2025-03-23",
  modules: ["@vueuse/nuxt"],
  css: ["~/assets/css/main.css"],
  vite: {
    plugins: [tailwindcss()],
    build: {
      sourcemap: false,
    },
  },
  nitro: {
    prerender: {
      routes: ["/"],
    },
  },
  app: {
    head: {
      script: [
        {
          async: true,
          src: "https://www.googletagmanager.com/gtag/js?id=G-093HLGSNY6",
        },
        {
          innerHTML: `window.dataLayer=window.dataLayer||[];function gtag(){dataLayer.push(arguments)}gtag("js",new Date());gtag("config","G-093HLGSNY6");`,
        },
        {
          innerHTML: `(function(){var s=localStorage.getItem("vueuse-color-scheme");var t=s==="light"?"light":s==="dark"?"dark":window.matchMedia("(prefers-color-scheme:light)").matches?"light":"dark";document.documentElement.classList.toggle("dark",t==="dark")})();`,
        },
      ],
      link: [
        { rel: "icon", type: "image/svg+xml", href: "/favicon.svg" },
        { rel: "preconnect", href: "https://fonts.googleapis.com" },
        {
          rel: "preconnect",
          href: "https://fonts.gstatic.com",
          crossorigin: "",
        },
        {
          href: "https://fonts.googleapis.com/css2?family=Orbitron:wght@400;500;600;700;800;900&family=DM+Sans:ital,opsz,wght@0,9..40,300;0,9..40,400;0,9..40,500;0,9..40,600;1,9..40,400&family=JetBrains+Mono:wght@400;500;600&display=swap",
          rel: "stylesheet",
        },
      ],
    },
  },
});
