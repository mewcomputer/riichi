import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { ArrowUpRight, LoaderCircle } from "lucide-react";

import { Button } from "@/components/ui/button";
import { ApiError, getCurrentUser } from "@/lib/api";
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
        </div>
      </section>
    </main>
  );
}
