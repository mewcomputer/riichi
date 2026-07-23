import themes from "@/themes-preview.json";

export const THEME_STORAGE_KEY = "riichi.theme.preferences";
export const THEME_OPTIONS = themes;
export type ThemeId = (typeof themes)[number]["id"] | "default";
export type ThemeMode = "system" | "light" | "dark";
export type ThemePreferences = {
  mode: ThemeMode;
  lightTheme: ThemeId;
  darkTheme: ThemeId;
};

const LIGHT_THEME_IDS = new Set<ThemeId>([
  "catppuccin-latte",
  "evergarden-summer",
  "rose-pine-dawn",
  "tokyo-night-light",
  "ayu-light",
]);

export const LIGHT_THEME_OPTIONS = themes.filter((theme) => LIGHT_THEME_IDS.has(theme.id));
export const DARK_THEME_OPTIONS = themes.filter((theme) => !LIGHT_THEME_IDS.has(theme.id));
export const DEFAULT_THEME_PREFERENCES: ThemePreferences = {
  mode: "system",
  lightTheme: "catppuccin-latte",
  darkTheme: "default",
};

function isThemeId(value: unknown): value is ThemeId {
  return value === "default" || (typeof value === "string" && themes.some((theme) => theme.id === value));
}

export function storedThemePreferences(): ThemePreferences {
  if (typeof window === "undefined") return DEFAULT_THEME_PREFERENCES;
  const raw = window.localStorage.getItem(THEME_STORAGE_KEY);
  if (raw) {
    try {
      const parsed = JSON.parse(raw) as Partial<ThemePreferences>;
      if ((parsed.mode === "system" || parsed.mode === "light" || parsed.mode === "dark") && isThemeId(parsed.lightTheme) && isThemeId(parsed.darkTheme)) {
        return { mode: parsed.mode, lightTheme: parsed.lightTheme, darkTheme: parsed.darkTheme };
      }
    } catch {
      // Fall through to the legacy value or defaults.
    }
  }
  const legacy = window.localStorage.getItem("riichi.theme");
  if (isThemeId(legacy)) {
    return LIGHT_THEME_IDS.has(legacy)
      ? { mode: "light", lightTheme: legacy, darkTheme: "default" }
      : { mode: "dark", lightTheme: "catppuccin-latte", darkTheme: legacy };
  }
  return DEFAULT_THEME_PREFERENCES;
}

export function applyThemePreferences(preferences: ThemePreferences) {
  if (typeof document === "undefined") return;
  const prefersDark = window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? true;
  const theme = preferences.mode === "light"
    ? preferences.lightTheme
    : preferences.mode === "dark"
      ? preferences.darkTheme
      : prefersDark ? preferences.darkTheme : preferences.lightTheme;
  if (theme === "default") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.dataset.theme = theme;
  window.localStorage.setItem(THEME_STORAGE_KEY, JSON.stringify(preferences));
}
