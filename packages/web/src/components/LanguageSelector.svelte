<script lang="ts">
  import {
    createI18n,
    getLocalizedPath,
    LOCALES,
    type Locale,
  } from "../lib/i18n";

  const {
    currentLocale,
    pathname,
  }: { currentLocale: Locale; pathname: string } = $props();

  const localeName = $derived.by(
    () => LOCALES.find((l) => l.code === currentLocale)?.name ?? currentLocale,
  );

  const m = $derived.by(() => createI18n(currentLocale));
</script>

<div
  translate="no"
  class="dropdown dropdown-end [view-transition-name:language-selector]"
>
  <div
    role="button"
    aria-label={m.language_selector_label()}
    title={m.language_selector_label()}
    tabindex="0"
    class="btn btn-sm btn-ghost"
  >
    <span aria-hidden="true" class="icon-[mdi--translate] text-xl"></span>
    <span
      aria-hidden="true"
      lang={currentLocale}
      class="max-sm:hidden before:content-[attr(data-locale)]"
      data-locale={localeName}
    ></span>
  </div>
  <ul
    tabindex="-1"
    class="dropdown-content z-1 menu p-2 shadow bg-base-100 rounded-box w-52"
  >
    {#each LOCALES as locale}
      {#if locale.code === currentLocale}
        <li class="disabled pointer-events-none">
          <a
            role="link"
            aria-current="true"
            aria-disabled="true"
            lang={locale.code}
            class="font-bold"
          >
            {locale.name}
          </a>
        </li>
      {:else}
        <li>
          <a
            href={getLocalizedPath(pathname, locale.code)}
            hreflang={locale.code}
            lang={locale.code}
            class:active={locale.code === currentLocale}
          >
            {locale.name}
          </a>
        </li>
      {/if}
    {/each}
  </ul>
</div>
