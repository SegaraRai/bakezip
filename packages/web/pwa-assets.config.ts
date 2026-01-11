import { defineConfig, type Preset } from "@vite-pwa/assets-generator/config";

const minimal2023PresetNoPadding: Preset = {
  transparent: {
    sizes: [64, 192, 512],
    favicons: [[48, "favicon.ico"]],
    padding: 0,
  },
  maskable: {
    sizes: [512],
    padding: 0,
  },
  apple: {
    sizes: [180],
    padding: 0,
  },
};

export default defineConfig({
  headLinkOptions: {
    preset: "2023",
  },
  preset: {
    ...minimal2023PresetNoPadding,
  },
  images: "public/favicon.svg",
});
