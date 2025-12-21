import svelte from "@astrojs/svelte";
import { paraglideVitePlugin } from "@inlang/paraglide-js";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "astro/config";
import { baseLocale, locales } from "./project.inlang/settings.json";

// https://astro.build/config
export default defineConfig({
  site: process.env.SITE_URL ?? "https://bakezip.roundtrip.dev",
  i18n: {
    locales: locales,
    defaultLocale: baseLocale,
    routing: {
      prefixDefaultLocale: false,
    },
  },
  integrations: [svelte()],
  vite: {
    plugins: [
      paraglideVitePlugin({
        project: "./project.inlang",
        outdir: "./src/paraglide",
      }),
      tailwindcss(),
    ],
    server: {
      watch: {
        ignored: ["**/crates/**", "**/target/**"],
      },
    },
  },
});
