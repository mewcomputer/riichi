import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { Outlet, useLocation, useNavigate } from '@tanstack/react-router'
import { LoaderCircle } from "@/lib/product-icons";

import { ApiError, getCurrentUser } from "@/lib/api";

export function RootRoute() {
  const location = useLocation();
  const navigate = useNavigate();
  const authQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const isLogin = location.pathname === "/login";
  const isLanding = location.pathname === "/";
  const isOnboarding = location.pathname === "/onboarding";
  const unauthenticated = authQuery.error instanceof ApiError && authQuery.error.status === 401;
  const needsOnboarding = Boolean(authQuery.data && authQuery.data.memberships.length === 0 && !isOnboarding);

  useEffect(() => {
    if (!isLogin && !isLanding && unauthenticated) void navigate({ to: "/login", replace: true });
    if (!isLogin && !isLanding && !isOnboarding && needsOnboarding) void navigate({ to: "/onboarding", replace: true });
  }, [isLogin, isLanding, isOnboarding, navigate, needsOnboarding, unauthenticated]);

  if (isLogin || isLanding) return <Outlet />;
  if (authQuery.isPending || !authQuery.data) {
    return <div className="grid min-h-svh place-items-center bg-background"><LoaderCircle className="size-5 animate-spin text-muted-foreground" /></div>;
  }
  return <Outlet />;
}
