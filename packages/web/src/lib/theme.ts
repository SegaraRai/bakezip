export type Theme = "light" | "dark" | "system";

export type StaticTheme = Exclude<Theme, "system">;

export const LSKEY_THEME = "theme";

export const DEFAULT_THEME: Theme = "system";

export const FALLBACK_THEME: StaticTheme = "light";

export const THEME_MAP: Record<StaticTheme, string> = {
  light: "winter",
  dark: "night",
};
