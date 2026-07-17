import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  ApiError,
  createAgentRole,
  getTeamAgentRoster,
  getCurrentUser,
  revokeAgentRole,
  revokeAgentSession,
} from "@/lib/api";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";
import { useActiveTeam } from "../hooks/use-active-team";
import { useHumanAgentRoster } from "../hooks/use-human-agent-roster";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

export function AgentsPage() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const { organizationSlug } = useParams({ from: "/$organizationSlug/agents" });
  const appLogout = useAppLogout();
  const [newRoleName, setNewRoleName] = useState("");
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const { activeTeam, selectTeam } = useActiveTeam(meQuery.data?.teams);
  const teamId = activeTeam?.team_id;
  const teamNavigation = navigationQuery.data?.organizations
    .flatMap((organization) => organization.teams)
    .find((team) => team.id === teamId);
  const agentProjectId = teamNavigation?.projects[0]?.id ?? projectId;
  const projectName = teamNavigation?.projects[0]?.name ?? activeMembership?.project_name ?? "riichi";
  const rosterQuery = useQuery({
    queryKey: ["agents", "team", teamId],
    queryFn: () => getTeamAgentRoster(teamId!),
    enabled: Boolean(teamId),
  });
  const replicatedRoster = useHumanAgentRoster(teamId);
  const revokeSessionMutation = useMutation({
    mutationFn: (session: { project_id: string; id: string }) => revokeAgentSession(session.project_id, session.id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["agents", "team", teamId] }),
  });
  const revokeRoleMutation = useMutation({
    mutationFn: (role: { project_id: string; id: string }) => revokeAgentRole(role.project_id, role.id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["agents", "team", teamId] }),
  });
  const createRoleMutation = useMutation({
    mutationFn: () => createAgentRole(agentProjectId!, {
      display_name: newRoleName.trim(),
      capabilities: ["comment", "request_spec", "discover", "complete", "release", "doc.read", "doc.apply_edit"],
    }),
    onSuccess: () => {
      setNewRoleName("");
      void queryClient.invalidateQueries({ queryKey: ["agents", "team", teamId] });
    },
  });

  const error = meQuery.error ?? (replicatedRoster ? null : rosterQuery.error);
  const roles = replicatedRoster?.roles ?? rosterQuery.data?.roles ?? [];
  const sessions = replicatedRoster?.sessions ?? rosterQuery.data?.sessions ?? [];
  const roleNames = new Map(roles.map((role) => [role.id, role.display_name]));

  return (
    <ProjectShell
      sidebar={<ProjectSidebar projectName={projectName} navigation={navigationQuery.data} memberships={meQuery.data?.memberships} activeProjectId={projectId} onProjectChange={selectProject} onLogout={appLogout} avatarUrl={meQuery.data?.avatar_url} onSearch={() => undefined} onNavigate={(label) => {
        if (label === "Issues") void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } });
        if (label === "Link GitHub") void navigate({ to: "/$organizationSlug/integrations", params: { organizationSlug } });
        if (label === "Invite people") void navigate({ to: "/$organizationSlug/settings", params: { organizationSlug } });
      }} userName={meQuery.data?.display_name ?? "Alex Morgan"} />}
    >
      <ProjectHeader view="all" views={[]} onViewChange={() => undefined} />
      <main className="mx-auto flex w-full max-w-screen-lg flex-col gap-6 px-8 py-8">
        <div className="flex items-end justify-between">
          <div>
            <Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="text-xs text-muted-foreground hover:text-foreground">← Queue</Link>
            <h1 className="mt-3 text-2xl font-medium tracking-tight">Agent roster</h1>
            <p className="mt-1 text-sm text-muted-foreground">{activeTeam?.team_name ?? "Team"} · roles, capability bounds, and live sessions.</p>
          </div>
          <div className="flex items-center gap-2">
            {meQuery.data?.teams.length ? (
              <Select value={activeTeam?.team_id} onValueChange={(value) => { if (value) selectTeam(value); }}>
                <SelectTrigger size="sm" aria-label="Active team"><SelectValue placeholder="Select team" /></SelectTrigger>
                <SelectContent>
                  {meQuery.data.teams.map((team) => <SelectItem key={team.team_id} value={team.team_id}>{team.team_name}</SelectItem>)}
                </SelectContent>
              </Select>
            ) : null}
            <Badge variant="outline">{sessions.filter((session) => session.state === "active").length} active</Badge>
          </div>
        </div>
        <div className="flex max-w-lg items-center gap-2 border-b border-border/60 pb-5">
          <Input aria-label="New role name" value={newRoleName} onChange={(event) => setNewRoleName(event.target.value)} placeholder="New agent role" className="h-8 text-xs" />
          <Button size="sm" onClick={() => createRoleMutation.mutate()} disabled={createRoleMutation.isPending || !newRoleName.trim() || !teamId || !agentProjectId}>Create role</Button>
        </div>
        {createRoleMutation.error ? <span className="text-xs text-destructive">{createRoleMutation.error.message}</span> : null}
        {error ? <div className="text-sm text-destructive">{error instanceof ApiError && error.status === 401 ? <a href="/auth/login" className="underline">Sign in</a> : error.message}</div> : null}
        <section className="grid gap-3">
          {roles.map((role) => {
            const roleSessions = sessions.filter((session) => session.agent_role_id === role.id);
            return (
              <article key={role.id} className="rounded-lg border border-border/70 bg-card/40 p-4">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <div className="flex items-center gap-2">
                      <h2 className="font-medium">{role.display_name}</h2>
                      {role.revoked_at ? <Badge variant="destructive">revoked</Badge> : <Badge variant="secondary">{role.active_session_count} active sessions</Badge>}
                    </div>
                    <p className="mt-1 text-xs text-muted-foreground">Role {role.id.slice(0, 8)} · owner {role.owner_account_id?.slice(0, 8) ?? "unassigned"}</p>
                    <div className="mt-3 flex flex-wrap gap-1">{role.capabilities.map((capability) => <Badge key={capability} variant="outline">{capability}</Badge>)}</div>
                  </div>
                  {!role.revoked_at ? <Button size="sm" variant="destructive" onClick={() => revokeRoleMutation.mutate(role)} disabled={revokeRoleMutation.isPending}>Revoke role</Button> : null}
                </div>
                <div className="mt-4 grid gap-2 border-t border-border/60 pt-3">
                  {roleSessions.map((session) => (
                    <div key={session.id} className="flex items-center justify-between text-xs">
                      <div>
                        <span className="font-mono">{session.id.slice(0, 12)}</span>
                        <span className="ml-2 text-muted-foreground">{roleNames.get(session.agent_role_id)} · {session.state}</span>
                      </div>
                      {session.state === "active" ? <Button size="sm" variant="ghost" className="h-7 text-destructive" onClick={() => revokeSessionMutation.mutate(session)} disabled={revokeSessionMutation.isPending}>Revoke session</Button> : null}
                    </div>
                  ))}
                  {roleSessions.length === 0 ? <span className="text-xs text-muted-foreground">No sessions recorded.</span> : null}
                </div>
              </article>
            );
          })}
          {!rosterQuery.isPending && roles.length === 0 ? <div className="rounded-lg border border-dashed p-8 text-sm text-muted-foreground">No agent roles yet.</div> : null}
        </section>
      </main>
    </ProjectShell>
  );
}
