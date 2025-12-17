import {m as orgM} from "../paraglide/messages";

export function createI18n(locale: string): typeof orgM {
  return Object.fromEntries(
    Object.entries(orgM).map(([key, func]) => [
      key,
      (params: any, options: any = {}) => func(params, {locale, ...options})
    ])
  ) as typeof orgM;
}
