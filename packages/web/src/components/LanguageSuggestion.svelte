<script lang="ts">
  import { onMount } from "svelte";
  import {
    createI18n,
    getLocalizedPath,
    LOCALES,
    type Locale,
  } from "../lib/i18n";

  const LSKEY_DISMISSED = "language-suggestion-dismissed";

  const {
    currentLocale,
    pathname,
  }: { currentLocale: Locale; pathname: string } = $props();

  let show = $state(false);
  let suggestedLocale = $state<(typeof LOCALES)[number] | null>(null);

  const m = $derived.by(() =>
    createI18n(suggestedLocale?.code ?? currentLocale),
  );

  onMount(() => {
    const dismissed = localStorage.getItem(LSKEY_DISMISSED);
    if (dismissed) {
      return;
    }

    const browserLocales = navigator.languages || [navigator.language];
    for (const locale of browserLocales) {
      const parsed = new Intl.Locale(locale);
      const matched =
        LOCALES.find((l) => l.code === `${parsed.language}-${parsed.region}`) ??
        LOCALES.find((l) => l.code === parsed.language);
      if (matched) {
        if (matched.code === currentLocale) {
          // Clear dismissed flag if user is already using the suggested language
          try {
            localStorage.removeItem(LSKEY_DISMISSED);
          } catch {}
        } else {
          suggestedLocale = matched;
          show = true;
        }
        // If the first matching locale is the current one, we are good.
        // If we found a match and it's different, we suggest.
        // If we found a match and it IS the current one, we stop looking.
        break;
      }
    }
  });

  function dismiss() {
    show = false;
    try {
      localStorage.setItem(LSKEY_DISMISSED, "true");
    } catch {}
  }
</script>

{#if show && suggestedLocale}
  <div lang={suggestedLocale.code} class="toast toast-bottom toast-center z-50">
    <div role="alert" aria-live="assertive" class="alert alert-info shadow-lg">
      <span aria-hidden="true" class="icon-[mdi--translate] text-2xl"></span>
      <div>
        <h3 class="font-bold">{m.language_suggestion_title()}</h3>
        <div class="text-xs">
          {m.language_suggestion_text({ language: suggestedLocale.name })}
        </div>
      </div>
      <div class="flex-none">
        <button class="btn btn-sm btn-ghost" onclick={dismiss}
          >{m.language_suggestion_dismiss()}</button
        >
        <a
          href={getLocalizedPath(pathname, suggestedLocale.code)}
          class="btn btn-sm btn-primary">{m.language_suggestion_accept()}</a
        >
      </div>
    </div>
  </div>
{/if}
