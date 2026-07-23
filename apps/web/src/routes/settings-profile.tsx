import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { SettingsShell } from "@/components/settings/settings-shell";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { getCurrentUser, updateThemePreferences, uploadAvatar } from "@/lib/api";
import {
  applyThemePreferences,
  DARK_THEME_OPTIONS,
  LIGHT_THEME_OPTIONS,
  storedThemePreferences,
  type ThemeMode,
  type ThemePreferences,
} from "@/lib/theme-preferences";

type ThemeOption = { id: string; label: string; preview?: readonly string[]; baseText?: string };

function ThemePreview({ option }: { option: ThemeOption }) {
  if (!option.preview || !option.baseText) return null;
  return <span className="inline-flex h-5 items-center gap-1 rounded-md border border-black/10 px-1.5 text-[11px] font-normal shadow-sm dark:border-white/10" style={{ backgroundColor: option.preview[0], color: option.baseText }}><span className="size-1.5 rounded-full" style={{ backgroundColor: option.preview[1] }} /><span className="size-1.5 rounded-full" style={{ backgroundColor: option.preview[2] }} /><span className="leading-none">Aa</span></span>;
}

function ThemeSelect({ label, value, options, onChange }: { label: string; value: string; options: ReadonlyArray<ThemeOption>; onChange: (value: string) => void }) {
  return <div className="grid gap-1.5 text-xs font-medium"><span>{label}</span><Select value={value} onValueChange={(nextValue) => { if (nextValue) onChange(nextValue); }}><SelectTrigger className="w-full font-normal" aria-label={label}><SelectValue /></SelectTrigger><SelectContent>{options.map((option) => <SelectItem key={option.id} value={option.id}><ThemePreview option={option} />{option.label}</SelectItem>)}</SelectContent></Select></div>;
}

export function SettingsProfilePage() {
  const queryClient = useQueryClient();
  const avatarInput = useRef<HTMLInputElement>(null);
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const avatarMutation = useMutation({ mutationFn: uploadAvatar, onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["auth", "me"] }) });
  const themeMutation = useMutation({ mutationFn: updateThemePreferences });
  const displayName = meQuery.data?.display_name ?? "Alex Morgan";
  const [preferences, setPreferences] = useState<ThemePreferences>(storedThemePreferences);
  const [themeHydrated, setThemeHydrated] = useState(false);
  useEffect(() => applyThemePreferences(preferences), [preferences]);
  useEffect(() => {
    if (themeHydrated || !meQuery.data) return;
    setPreferences({ mode: meQuery.data.theme_mode, lightTheme: meQuery.data.light_theme as ThemePreferences["lightTheme"], darkTheme: meQuery.data.dark_theme as ThemePreferences["darkTheme"] });
    setThemeHydrated(true);
  }, [meQuery.data, themeHydrated]);
  const updatePreferences = (change: Partial<ThemePreferences>) => {
    const next = { ...preferences, ...change };
    setPreferences(next);
    themeMutation.mutate({ mode: next.mode, light_theme: next.lightTheme, dark_theme: next.darkTheme });
  };

  return <SettingsShell>
    <header className="mb-8"><p className="text-xs text-muted-foreground">Your account</p><h1 className="mt-2 text-2xl font-medium tracking-tight">Profile</h1><p className="mt-2 text-sm text-muted-foreground">Your identity and preferences across Riichi.</p></header>
    <div className="grid max-w-3xl gap-8">
      <section className="grid gap-5 rounded-lg border border-border/70 bg-card/20 p-5">
        <div className="flex items-center gap-4"><Avatar key={meQuery.data?.avatar_url ?? "fallback"} size="lg" className="animate-in zoom-in-95 duration-200">{meQuery.data?.avatar_url ? <AvatarImage src={meQuery.data.avatar_url} alt="" /> : null}<AvatarFallback>{displayName.split(" ").map((part) => part[0]).join("")}</AvatarFallback></Avatar><div className="grid gap-1"><span className="text-sm font-medium">{displayName}</span><span className="text-xs text-muted-foreground">{meQuery.data?.email ?? "No email available"}</span></div><input ref={avatarInput} type="file" accept="image/jpeg,image/png,image/webp,image/gif" className="hidden" onChange={(event) => { const file = event.target.files?.[0]; if (file) avatarMutation.mutate(file); event.target.value = ""; }} /><Button className="ml-auto" variant="outline" size="sm" onClick={() => avatarInput.current?.click()} disabled={avatarMutation.isPending}>{avatarMutation.isPending ? "Uploading…" : "Change image"}</Button></div>
        {avatarMutation.error ? <p role="alert" className="text-xs text-destructive">Could not update your image: {avatarMutation.error.message}</p> : null}
      </section>
      <section className="grid gap-5">
        <div><h2 className="text-sm font-medium">Appearance</h2><p className="mt-1 text-xs text-muted-foreground">Choose which light and dark palettes to use. System mode follows your device preference.</p></div>
        <div className="grid gap-4 rounded-lg border border-border/70 bg-card/20 p-5">
          <ThemeSelect label="Theme mode" value={preferences.mode} options={[{ id: "system", label: "System preference" }, { id: "light", label: "Light" }, { id: "dark", label: "Dark" }]} onChange={(value) => updatePreferences({ mode: value as ThemeMode })} />
          <div className="grid gap-4 sm:grid-cols-2"><ThemeSelect label="Light theme" value={preferences.lightTheme} options={[{ id: "default", label: "Riichi default", preview: ["#171722", "#94e2d5", "#cdd6f4"], baseText: "#cdd6f4" }, ...LIGHT_THEME_OPTIONS.map((option) => ({ id: option.id, label: `${option.category} · ${option.name}`, preview: option.preview, baseText: option.baseText }))]} onChange={(value) => updatePreferences({ lightTheme: value as ThemePreferences["lightTheme"] })} /><ThemeSelect label="Dark theme" value={preferences.darkTheme} options={[{ id: "default", label: "Riichi default", preview: ["#171722", "#94e2d5", "#cdd6f4"], baseText: "#cdd6f4" }, ...DARK_THEME_OPTIONS.map((option) => ({ id: option.id, label: `${option.category} · ${option.name}`, preview: option.preview, baseText: option.baseText }))]} onChange={(value) => updatePreferences({ darkTheme: value as ThemePreferences["darkTheme"] })} /></div>
          <p className="text-xs text-muted-foreground">{preferences.mode === "system" ? "Using your operating system’s current appearance." : preferences.mode === "light" ? "Using your selected light theme." : "Using your selected dark theme."}{themeMutation.isPending ? " Saving…" : themeMutation.isSuccess ? " Saved to your profile." : ""}</p>
          {themeMutation.error ? <p role="alert" className="text-xs text-destructive">Could not save your theme preference: {themeMutation.error.message}</p> : null}
        </div>
      </section>
    </div>
  </SettingsShell>;
}
