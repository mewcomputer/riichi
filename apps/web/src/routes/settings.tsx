import { useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import { ApiError, createInvite, getCurrentUser, uploadAvatar, uploadOrganizationLogo } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";
import { TeamEmojiEditor } from "../components/team/team-emoji-editor";

export function SettingsPage() {
  const navigate = useNavigate();
  const { organizationSlug } = useParams({ from: "/$organizationSlug/settings" });
  const appLogout = useAppLogout();
  const queryClient = useQueryClient();
  const avatarInput = useRef<HTMLInputElement>(null);
  const organizationLogoInput = useRef<HTMLInputElement>(null);
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<"viewer" | "member" | "admin">("member");
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const projectName = activeMembership?.project_name ?? "riichi";
  const canManageProject = activeMembership?.role === "owner" || activeMembership?.role === "admin";
  const organization = navigationQuery.data?.organizations[0];
  const teamMemberships = new Map((meQuery.data?.teams ?? []).map((team) => [team.team_id, team]));
  const canManageAnyTeam = organization?.teams.some((team) => {
    const role = teamMemberships.get(team.id)?.role;
    return role === "owner" || role === "admin";
  }) ?? false;
  const canManageOrganization = organization?.role === "owner" || organization?.role === "admin";
  const avatarMutation = useMutation({
    mutationFn: uploadAvatar,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["auth", "me"] }),
  });
  const organizationLogoMutation = useMutation({
    mutationFn: (file: File) => uploadOrganizationLogo(organization!.id, file),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["navigation"] }),
  });
  const inviteMutation = useMutation({
    mutationFn: () => createInvite(projectId!, { role, email_hint: email.trim() || undefined }),
    onSuccess: () => setEmail(""),
  });

  return (
    <ProjectShell sidebar={<ProjectSidebar projectName={projectName} navigation={navigationQuery.data} memberships={meQuery.data?.memberships} activeProjectId={projectId} onProjectChange={selectProject} onLogout={appLogout} avatarUrl={meQuery.data?.avatar_url} onSearch={() => undefined} onNavigate={(label) => {
      if (label === "Issues") void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } });
      if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
      if (label === "Link GitHub") void navigate({ to: "/$organizationSlug/integrations", params: { organizationSlug } });
    }} userName={meQuery.data?.display_name ?? "Alex Morgan"} />}>
      <ProjectHeader view="all" views={[]} onViewChange={() => undefined} />
      <main className="mx-auto flex w-full max-w-screen-lg flex-col gap-6 px-8 py-8">
        <div>
          <Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="text-xs text-muted-foreground hover:text-foreground">← Issues</Link>
          <h1 className="mt-3 text-2xl font-medium tracking-tight">Settings</h1>
          <p className="mt-1 text-sm text-muted-foreground">Personal, organization, team, and project settings in one place.</p>
        </div>
        <section className="grid gap-4 border-y border-border/60 py-5">
          <div>
            <h2 className="text-sm font-medium">Personal</h2>
            <p className="mt-1 text-xs text-muted-foreground">Your profile image and account identity.</p>
          </div>
          <div className="flex items-center gap-3">
            <Avatar key={meQuery.data?.avatar_url ?? "fallback"} size="lg" className="animate-in zoom-in-95 duration-200">
              {meQuery.data?.avatar_url ? <AvatarImage src={meQuery.data.avatar_url} alt="" /> : null}
              <AvatarFallback>{(meQuery.data?.display_name ?? "Alex Morgan").split(" ").map((part) => part[0]).join("")}</AvatarFallback>
            </Avatar>
            <div className="grid gap-1">
              <span className="text-sm font-medium">{meQuery.data?.display_name ?? "Alex Morgan"}</span>
              <span className="text-xs text-muted-foreground">{meQuery.data?.email ?? "No email available"}</span>
            </div>
            <input ref={avatarInput} type="file" accept="image/jpeg,image/png,image/webp,image/gif" className="hidden" onChange={(event) => { const file = event.target.files?.[0]; if (file) avatarMutation.mutate(file); event.target.value = ""; }} />
            <Button className="ml-auto" variant="outline" size="sm" onClick={() => avatarInput.current?.click()} disabled={avatarMutation.isPending}>Change image</Button>
          </div>
        </section>
        {(canManageProject || canManageAnyTeam) && organization ? <section className="grid gap-4 border-b border-border/60 pb-5">
          <div>
            <h2 className="text-sm font-medium">Organization</h2>
            <p className="mt-1 text-xs text-muted-foreground">{organization.name} access and team structure.</p>
          </div>
          <div className="flex items-center gap-3">
            {organization.logo_url ? <img src={organization.logo_url} alt="" className="size-12 rounded-lg border border-border/60 object-cover" /> : <div className="grid size-12 place-items-center rounded-lg border border-border/60 text-sm font-semibold">{organization.name.slice(0, 1).toUpperCase()}</div>}
            <div className="grid gap-1"><span className="text-xs font-medium">Organization image</span><span className="text-xs text-muted-foreground">PNG or SVG, up to 2 MB.</span></div>
            {canManageOrganization ? <><input ref={organizationLogoInput} type="file" accept="image/png,image/svg+xml" className="hidden" onChange={(event) => { const file = event.target.files?.[0]; if (file) organizationLogoMutation.mutate(file); event.target.value = ""; }} /><Button className="ml-auto" variant="outline" size="sm" onClick={() => organizationLogoInput.current?.click()} disabled={organizationLogoMutation.isPending}>{organizationLogoMutation.isPending ? "Uploading…" : "Change image"}</Button></> : null}
          </div>
          {organizationLogoMutation.error ? <span className="text-xs text-destructive">{organizationLogoMutation.error.message}</span> : null}
          <div className="grid gap-2">
            {organization.teams.map((team) => <div key={team.id} className="flex items-center justify-between gap-4 rounded-md border border-border/60 px-3 py-2 text-xs"><div className="flex min-w-0 items-center gap-2"><TeamEmojiEditor team={team} canManage={teamMemberships.get(team.id)?.role === "owner" || teamMemberships.get(team.id)?.role === "admin"} onSaved={() => void navigationQuery.refetch()} /><span className="font-medium">{team.name}</span><span className="text-muted-foreground">{team.key} · {team.projects.length} project{team.projects.length === 1 ? "" : "s"}</span></div></div>)}
          </div>
        </section> : null}
        {canManageProject ? <section className="grid gap-5 border-b border-border/60 pb-5">
          <div>
            <h2 className="text-sm font-medium">Project · {projectName}</h2>
            <p className="mt-1 text-xs text-muted-foreground">Access, invitations, and integrations for the active project.</p>
          </div>
          <div className="grid gap-4">
            <div className="grid gap-4">
              <h3 className="text-xs font-medium">Invite a collaborator</h3>
              <div className="flex items-end gap-2">
                <label className="grid flex-1 gap-1.5 text-xs text-muted-foreground">Email hint
                  <Input aria-label="Invite email" value={email} onChange={(event) => setEmail(event.target.value)} placeholder="person@example.com" className="h-8 text-xs" />
                </label>
                <label className="grid w-28 gap-1.5 text-xs text-muted-foreground">Role
                  <select aria-label="Invite role" value={role} onChange={(event) => setRole(event.target.value as typeof role)} className="h-8 rounded-md border border-input bg-background px-2 text-xs">
                    <option value="viewer">Viewer</option><option value="member">Member</option><option value="admin">Admin</option>
                  </select>
                </label>
                <Button size="sm" onClick={() => inviteMutation.mutate()} disabled={inviteMutation.isPending || !projectId}>Create invite</Button>
              </div>
              {inviteMutation.error ? <span className="text-xs text-destructive">{inviteMutation.error instanceof ApiError && inviteMutation.error.status === 403 ? "Admin access is required." : inviteMutation.error.message}</span> : null}
              {inviteMutation.data ? <div className="grid gap-2 rounded-md border border-border/70 p-3 text-xs"><Badge variant="secondary">Invite created</Badge><span className="text-muted-foreground">Share this one-time token with the invitee:</span><code className="break-all rounded bg-muted px-2 py-1">{inviteMutation.data.token}</code></div> : null}
            </div>
            <div className="flex items-center justify-between border-t border-border/60 pt-4">
              <div><h3 className="text-xs font-medium">GitHub integration</h3><p className="mt-1 text-xs text-muted-foreground">Import bounded external issue snapshots.</p></div>
              <Button variant="outline" size="sm" render={<Link to="/$organizationSlug/integrations" params={{ organizationSlug }} />}>Open integration</Button>
            </div>
          </div>
        </section> : null}
      </main>
    </ProjectShell>
  );
}
