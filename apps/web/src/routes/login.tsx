import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowUpRight, LoaderCircle } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ApiError, getCurrentUser, getApiBaseUrl, setApiBaseUrl } from "@/lib/api";
import { advanceShortcut } from "@/lib/keyboard-shortcuts";
import SusukiMoonSvg from "@/components/susuki_moon";
import { useNavigation } from "../hooks/use-navigation";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";

export function LoginPage() {
  const navigate = useNavigate();
  const authQuery = useQuery({
    queryKey: ["auth", "me"],
    queryFn: getCurrentUser,
    retry: false,
  });
  const navigationQuery = useNavigation();

  useEffect(() => {
    if (authQuery.data && navigationQuery.data) void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug: toOrganizationSlug(navigationQuery.data.organizations[0]?.name ?? "Riichi") }, replace: true });
  }, [authQuery.data, navigate, navigationQuery.data]);

  const [showApiUrlInput, setShowApiUrlInput] = useState(false);
  const [apiUrlValue, setApiUrlValue] = useState("");

  useEffect(() => {
    if (showApiUrlInput) return;
    const buffer: string[] = [];
    let timeout: number | undefined;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']")) return;
      const result = advanceShortcut(buffer, event.key, [["a", "u"]]);
      buffer.splice(0, buffer.length, ...result.buffer);
      if (timeout !== undefined) window.clearTimeout(timeout);
      if (result.buffer.length > 0) {
        event.preventDefault();
        timeout = window.setTimeout(() => buffer.splice(0), 900);
      }
      if (result.matched) {
        event.preventDefault();
        setApiUrlValue(getApiBaseUrl());
        setShowApiUrlInput(true);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      if (timeout !== undefined) window.clearTimeout(timeout);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [showApiUrlInput]);

  if (authQuery.isPending) {
    return (
      <div className="grid min-h-svh place-items-center bg-background">
        <LoaderCircle className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }
  if (authQuery.data) {
    return null;
  }

  const authUnavailable =
    authQuery.error &&
    (!(authQuery.error instanceof ApiError) || authQuery.error.status !== 401);

  return (
    <main className="grid min-h-svh place-items-center bg-background px-6 text-foreground">
      <section className="w-full max-w-sm">
        <SusukiMoonSvg className="size-24 mb-1" />
        <div className="py-2">
          <h1 className="text-2xl font-medium tracking-tight">
            Log in to Riichi
          </h1>
          <Button
            className="mt-4 w-full justify-between"
            render={<a href="/auth/login" />}
          >
            Continue with Pocket ID <ArrowUpRight />
          </Button>
          {authUnavailable ? (
            <p className="mt-4 text-xs text-destructive">
              Authentication is unavailable right now. Try again in a moment.
            </p>
          ) : null}
          {showApiUrlInput ? (
            <div className="mt-4 grid gap-2">
              <label className="text-xs text-muted-foreground">API endpoint URL</label>
              <Input
                aria-label="API endpoint URL"
                value={apiUrlValue}
                onChange={(event) => setApiUrlValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") setApiBaseUrl(apiUrlValue);
                  if (event.key === "Escape") setShowApiUrlInput(false);
                }}
                placeholder="https://api.example.com"
                className="h-8 text-xs"
                autoFocus
              />
              <div className="flex items-center justify-between">
                <button
                  type="button"
                  className="text-xs text-muted-foreground hover:text-foreground"
                  onClick={() => { setApiUrlValue(""); setApiBaseUrl(""); }}
                >
                  Reset to default
                </button>
                <Button size="sm" onClick={() => setApiBaseUrl(apiUrlValue)}>Save</Button>
              </div>
              <p className="text-xs text-muted-foreground">Leave empty to use the default. The page reloads on save.</p>
            </div>
          ) : null}
        </div>
      </section>
    </main>
  );
}
