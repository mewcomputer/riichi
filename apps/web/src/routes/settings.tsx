import { Link, useParams } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";

import { SettingsShell } from "@/components/settings/settings-shell";
import { getCurrentUser } from "@/lib/api";
import { useActiveProject } from "@/hooks/use-active-project";
import { useNavigation } from "@/hooks/use-navigation";

function SettingsCard({ to, title, description, detail }: { to: string; title: string; description: string; detail?: string }) {
  return <Link to={to as never} className="group flex items-center justify-between gap-4 rounded-lg border border-border/70 bg-card/20 px-4 py-3 transition-colors hover:bg-muted/30"><div className="min-w-0"><p className="text-sm font-medium">{title}</p><p className="mt-0.5 text-xs text-muted-foreground">{description}</p></div><span className="shrink-0 text-xs text-muted-foreground group-hover:text-foreground">{detail ?? "›"}</span></Link>;
}

export function SettingsOverviewPage() {
  const { organizationSlug } = useParams({ from: "/$organizationSlug/settings" });
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership } = useActiveProject(meQuery.data?.memberships);
  const organization = navigationQuery.data?.organizations[0];

  return <SettingsShell>
    <header className="mb-10"><p className="text-xs text-muted-foreground">Settings</p><h1 className="mt-2 text-3xl font-medium tracking-tight">Workspace settings</h1><p className="mt-2 max-w-2xl text-sm text-muted-foreground">Manage your account, workspace, teams, and the active project.</p></header>
    <div className="grid max-w-2xl gap-8">
      <section className="grid gap-3"><h2 className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground/70">Your account</h2><SettingsCard to={`/${organizationSlug}/settings/profile`} title="Profile" description="Your name, email, and profile image" /></section>
      {organization ? <section className="grid gap-3"><h2 className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground/70">Workspace</h2><SettingsCard to={`/${organizationSlug}/settings/organization`} title={organization.name} description="Workspace identity and team structure" detail={`${organization.teams.length} team${organization.teams.length === 1 ? "" : "s"}`} />{organization.teams.map((team) => <SettingsCard key={team.id} to={`/${organizationSlug}/teams/${team.key}/settings`} title={team.name} description={`Team settings · ${team.key}`} detail={`${team.projects.length} project${team.projects.length === 1 ? "" : "s"}`} />)}</section> : null}
      {activeMembership ? <section className="grid gap-3"><h2 className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground/70">Active project</h2><SettingsCard to={`/${organizationSlug}/settings/project`} title={activeMembership.project_name} description="Access, invitations, and integrations for the active project" /></section> : null}
    </div>
  </SettingsShell>;
}
