import { m as orgM } from "../paraglide/messages";
import { baseLocale, type Locale, locales } from "../paraglide/runtime";

export type { Locale };

export const LOCALES: readonly {
  readonly code: Locale;
  readonly name: string;
}[] = [
  { code: "en", name: "English" },
  { code: "ja", name: "日本語" },
];

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
  const strippedPath = path.replace(/^\/+/, "");
  const localeStrippedPath = (locales as readonly string[]).includes(
    strippedPath.split("/")[0],
  )
    ? strippedPath.split("/").slice(1).join("/")
    : strippedPath;
  return effectiveLocale === baseLocale
    ? `/${localeStrippedPath}`
    : `/${effectiveLocale}/${localeStrippedPath}`.replace(/\/$/, "");
}
