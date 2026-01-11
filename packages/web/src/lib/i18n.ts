import { m as orgM } from "../paraglide/messages";
import { baseLocale, type Locale, locales } from "../paraglide/runtime";

export type { Locale } from "../paraglide/runtime";

export const DEFAULT_LOCALE: Locale = baseLocale;

export const LSKEY_SELECTED_LOCALE = "user-selected-language";

export const LOCALES: readonly {
  readonly code: Locale;
  readonly name: string;
}[] = [
  { code: "en", name: "English" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "zh-CN", name: "简体中文" },
  { code: "zh-TW", name: "繁體中文" },
] satisfies {
  [P in Extract<keyof typeof locales, `${number}`>]: {
    code: (typeof locales)[P];
    name: string;
  };
};

function toAvailableLocale(locale: string): Locale {
  if ((locales as readonly string[]).includes(locale)) {
    return locale as Locale;
  }
  return baseLocale;
}

export function createI18n(locale: Locale): typeof orgM {
  const effectiveLocale = toAvailableLocale(locale);

  return Object.fromEntries(
    Object.entries(orgM).map(([key, func]) => [
      key,
      (params: any, options: any = {}) =>
        func(params, { locale: effectiveLocale, ...options }),
    ]),
  ) as typeof orgM;
}

export function getLocalizedPath(path: string, locale: Locale): string {
  const effectiveLocale = toAvailableLocale(locale);

  const components = path
    .replace(/\.html$/, "")
    .replace(/^\/+/, "")
    .split("/");
  const localeStrippedPath = (locales as readonly string[]).includes(
    components[0],
  )
    ? components.slice(1).join("/")
    : components.join("/");

  return effectiveLocale === baseLocale
    ? `/${localeStrippedPath}`
    : `/${effectiveLocale}/${localeStrippedPath}`.replace(/\/+$/, "");
}
