<script lang="ts">
  import { onMount } from "svelte";
  import {
    createI18n,
    DEFAULT_LOCALE,
    getLocalizedPath,
    LOCALES,
    LSKEY_SELECTED_LOCALE,
    type Locale,
  } from "../lib/i18n";

  let locale = $state<Locale>(DEFAULT_LOCALE);

  const m = $derived.by(() => createI18n(locale));

  onMount(() => {
    let language;
    try {
      language ??= document.documentElement.dataset.lastLocale;
      language ??= localStorage.getItem(LSKEY_SELECTED_LOCALE);
    } catch {}

    const languages = language
      ? [language]
      : (navigator.languages ?? [navigator.language]);

    let finalLanguage = DEFAULT_LOCALE;
    for (const language of languages) {
      const parsed = new Intl.Locale(language);
      const matched =
        LOCALES.find((l) => l.code === `${parsed.language}-${parsed.region}`) ??
        LOCALES.find((l) => l.code === parsed.language);
      if (matched) {
        finalLanguage = matched.code;
        break;
      }
    }

    locale = finalLanguage;
  });
</script>

<a
  lang={locale}
  href={getLocalizedPath("/", locale)}
  hreflang={locale}
  class="btn btn-ghost gap-2 pl-0"
>
  <span class="icon-[mdi--arrow-left] text-xl"></span>
  <span>{m.back_button_text()}</span>
</a>
