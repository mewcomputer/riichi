import type { ReactNode } from "react";

import { Link, useLocation } from "@tanstack/react-router";

import { useQuery } from "@tanstack/react-query";
import { getCurrentUser } from "@/lib/api";
import { useActiveProject } from "@/hooks/use-active-project";
import { useNavigation } from "@/hooks/use-navigation";
import { organizationSlug as toOrganizationSlug } from "@/lib/organization-slug";

export function SettingsShell({ children }: { children: ReactNode }) {
  const location = useLocation();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership, projectId } = useActiveProject(meQuery.data?.memberships);
  const organization = navigationQuery.data?.organizations[0];
  const organizationSlug = toOrganizationSlug(organization?.name ?? "riichi");
  const team = organization?.teams.find((candidate) => candidate.projects.some((project) => project.id === projectId));
  const isActive = (path: string) => location.pathname === path;

  return (
    <div className="min-h-svh bg-background text-foreground">
      <aside className="fixed inset-y-0 left-0 z-20 hidden w-60 border-r border-border/70 bg-sidebar lg:flex lg:flex-col">
        <div className="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto px-3 py-4">
          <Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="px-2 text-xs text-muted-foreground hover:text-foreground">← Back to app</Link>
          <div className="grid gap-1">
            <p className="px-2 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground/70">Settings</p>
            <Link to="/$organizationSlug/settings/overview" params={{ organizationSlug }} className={`rounded-md px-2 py-1.5 text-sm ${isActive(`/${organizationSlug}/settings/overview`) ? "bg-sidebar-accent font-medium text-sidebar-accent-foreground" : "text-sidebar-foreground/75 hover:bg-sidebar-accent/70 hover:text-sidebar-foreground"}`}>Overview</Link>
            <Link to="/$organizationSlug/settings" params={{ organizationSlug }} className={`rounded-md px-2 py-1.5 text-sm ${isActive(`/${organizationSlug}/settings`) || isActive(`/${organizationSlug}/settings/profile`) ? "bg-sidebar-accent font-medium text-sidebar-accent-foreground" : "text-sidebar-foreground/75 hover:bg-sidebar-accent/70 hover:text-sidebar-foreground"}`}>Profile</Link>
          </div>
          {organization ? <div className="grid gap-1">
            <p className="px-2 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground/70">Workspace</p>
            <Link to="/$organizationSlug/settings/organization" params={{ organizationSlug }} className={`rounded-md px-2 py-1.5 text-sm ${isActive(`/${organizationSlug}/settings/organization`) ? "bg-sidebar-accent font-medium text-sidebar-accent-foreground" : "text-sidebar-foreground/75 hover:bg-sidebar-accent/70 hover:text-sidebar-foreground"}`}>{organization.name}</Link>
            <div className="mt-3 grid gap-1">
              <p className="px-2 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground/70">Teams</p>
              {organization.teams.map((candidate) => <Link key={candidate.id} to="/$organizationSlug/teams/$teamKey/settings" params={{ organizationSlug, teamKey: candidate.key }} className="rounded-md px-2 py-1.5 text-sm text-sidebar-foreground/75 hover:bg-sidebar-accent/70 hover:text-sidebar-foreground">{candidate.name}</Link>)}
            </div>
          </div> : null}
          {activeMembership && team ? <div className="grid gap-1">
            <p className="px-2 text-[11px] font-medium uppercase tracking-[0.14em] text-muted-foreground/70">Active project</p>
            <Link to="/$organizationSlug/settings/project" params={{ organizationSlug }} className={`rounded-md px-2 py-1.5 text-sm ${isActive(`/${organizationSlug}/settings/project`) ? "bg-sidebar-accent font-medium text-sidebar-accent-foreground" : "text-sidebar-foreground/75 hover:bg-sidebar-accent/70 hover:text-sidebar-foreground"}`}>{activeMembership.project_name}</Link>
          </div> : null}
        </div>
      </aside>
      <div className="min-h-svh lg:pl-60">
        <header className="flex h-14 items-center border-b border-border/70 px-5 lg:hidden">
          <Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="text-xs text-muted-foreground hover:text-foreground">← Back to app</Link>
        </header>
        <main className="mx-auto w-full max-w-5xl px-5 py-8 sm:px-8 lg:px-12 lg:py-12">{children}</main>
      </div>
    </div>
  );
}
