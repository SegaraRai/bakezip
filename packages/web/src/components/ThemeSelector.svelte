<script lang="ts">
  import { onMount } from "svelte";
  import { createI18n, type Locale } from "../lib/i18n";

  type Theme = "light" | "dark" | "system";

  const LSKEY_THEME = "theme";
  const THEMES: readonly { readonly theme: Theme; readonly icon: string }[] = [
    { theme: "light", icon: "icon-[mdi--white-balance-sunny]" },
    { theme: "dark", icon: "icon-[mdi--weather-night]" },
    { theme: "system", icon: "icon-[mdi--desktop-mac]" },
  ];
  const DEFAULT_THEME: Theme = "system";
  const THEME_MAP: Partial<Record<Theme, string>> = {
    light: "winter",
    dark: "night",
  };

  const { locale }: { locale: Locale } = $props();

  let theme = $state<Theme | null>(null);

  const m = $derived.by(() => createI18n(locale));

  onMount(() => {
    const stored = localStorage.getItem(LSKEY_THEME);
    theme = (
      stored && THEMES.some(({ theme }) => theme === stored)
        ? (stored as Theme)
        : DEFAULT_THEME
    ) as Theme;
  });

  $effect(() => {
    if (!theme) {
      // Not initialized yet
      return;
    }

    try {
      localStorage.setItem(LSKEY_THEME, theme);
    } catch {}

    const applyTheme = (newTheme: Exclude<Theme, "system">) => {
      const themeName = THEME_MAP[newTheme] ?? newTheme;
      document.documentElement.dataset.theme = themeName;
    };

    if (theme !== "system") {
      applyTheme(theme);
      return;
    }

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      applyTheme(mediaQuery.matches ? "dark" : "light");
    };
    mediaQuery.addEventListener("change", handleChange);
    handleChange();

    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  });

  function setTheme(t: Theme): void {
    theme = t;
  }

  function getThemeName(t: Theme) {
    switch (t) {
      case "light":
        return m.theme_light();
      case "dark":
        return m.theme_dark();
      case "system":
        return m.theme_system();
    }
  }
</script>

<div class="dropdown dropdown-end [view-transition-name:theme-selector]">
  <div
    role="button"
    aria-label={m.theme_selector_label()}
    title={m.theme_selector_label()}
    tabindex="0"
    class="btn btn-sm btn-ghost btn-circle"
  >
    <span aria-hidden="true" class="icon-[mdi--theme-light-dark] text-xl"
    ></span>
  </div>
  {#if theme}
    <ul
      tabindex="-1"
      class="dropdown-content z-1 menu p-2 shadow bg-base-100 rounded-box w-52"
    >
      {#each THEMES as { theme: t, icon }}
        <li
          class:disabled={theme === t}
          class="[.disabled]:pointer-events-none"
        >
          <button
            disabled={theme === t}
            class="disabled:font-bold disabled:pointer-events-none"
            onclick={() => setTheme(t)}
          >
            <span aria-hidden="true" class={icon}></span>
            {getThemeName(t)}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
