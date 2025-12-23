import fs from "node:fs/promises";
import { chromium } from "playwright";

const LANGS = ["en", "ja", "zh-CN", "zh-TW", "ko"];

const svgContent = await fs.readFile(
  new URL("social.svg", import.meta.url),
  "utf-8",
);

const browser = await chromium.launch();
const page = await browser.newPage();

await page.setViewportSize({ width: 1200 + 100, height: 630 + 100 });
await page.setContent(svgContent);

const svg = page.locator("svg");

for (const lang of LANGS) {
  await svg.evaluate((node, lang) => {
    node.dataset.displayLang = lang;
  }, lang);

  const data = await svg.screenshot({ type: "png" });
  await fs.writeFile(
    new URL(`public/social-${lang}.png`, import.meta.url),
    data,
  );
}

await browser.close();
