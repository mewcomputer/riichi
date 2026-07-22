import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import { getCurrentUser } from "@/lib/api";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { TeamEmojiEditor } from "@/components/team/team-emoji-editor";
import { TeamMark } from "@/components/team/team-mark";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";

export function TeamSettingsPage() {
  const { organizationSlug, teamKey } = useParams({ from: "/$organizationSlug/teams/$teamKey/settings" });
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const queryClient = useQueryClient();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const organization = navigationQuery.data?.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
  const team = organization?.teams.find((candidate) => candidate.key.toLowerCase() === teamKey.toLowerCase());
  const teamMembership = meQuery.data?.teams.find((candidate) => candidate.team_id === team?.id);
  const canManage = teamMembership?.role === "owner" || teamMembership?.role === "admin";

  if (!team) return <div className="p-8 text-sm text-muted-foreground">Loading team…</div>;
  return (
    <ProjectShell
      sidebar={
        <ProjectSidebar
          projectName={activeMembership?.project_name ?? "riichi"}
          navigation={navigationQuery.data}
          memberships={meQuery.data?.memberships}
          activeProjectId={projectId}
          onProjectChange={selectProject}
          onLogout={appLogout}
          avatarUrl={meQuery.data?.avatar_url}
          onSearch={() => undefined}
          onNavigate={(label) => {
            if (label === "Issues") void navigate({ to: "/$organizationSlug/teams/$teamKey/issues", params: { organizationSlug, teamKey: team.key } });
            if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
          }}
          userName={meQuery.data?.display_name ?? "Alex Morgan"}
        />
      }
    >
      <ProjectHeader view="all" views={[]} onViewChange={() => undefined} content={<div className="flex items-center gap-2 text-xs"><Link to="/$organizationSlug/teams/$teamKey" params={{ organizationSlug, teamKey: team.key }} className="text-muted-foreground hover:text-foreground">{team.name}</Link><span className="text-muted-foreground/40">›</span><span>Settings</span></div>} />
      <main className="mx-auto flex w-full max-w-screen-lg flex-col gap-8 px-8 py-10">
        <header><h1 className="text-2xl font-medium tracking-tight">Team settings</h1><p className="mt-1 text-sm text-muted-foreground">Identity and access for {team.name}.</p></header>
        <section className="grid gap-4 border-y border-border/60 py-6">
          <div><h2 className="text-sm font-medium">Identity</h2><p className="mt-1 text-xs text-muted-foreground">This emoji appears beside the team throughout Riichi.</p></div>
          <div className="flex items-center justify-between rounded-md border border-border/60 px-3 py-3"><div className="flex items-center gap-3"><TeamMark value={team.emoji} className="size-5" /><div className="grid gap-1"><span className="text-sm font-medium">{team.name}</span><span className="font-mono text-xs text-muted-foreground">{team.key}</span></div></div><TeamEmojiEditor team={team} canManage={canManage} onSaved={() => void queryClient.invalidateQueries({ queryKey: ["navigation"] })} /></div>
        </section>
      </main>
    </ProjectShell>
  );
}
