import { useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { SettingsShell } from "@/components/settings/settings-shell";
import { Button } from "@/components/ui/button";
import { getCurrentUser, uploadOrganizationLogo } from "@/lib/api";
import { TeamEmojiEditor } from "@/components/team/team-emoji-editor";
import { useNavigation } from "@/hooks/use-navigation";

export function SettingsOrganizationPage() {
  const queryClient = useQueryClient();
  const organizationLogoInput = useRef<HTMLInputElement>(null);
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const organization = navigationQuery.data?.organizations[0];
  const teamMemberships = new Map((meQuery.data?.teams ?? []).map((team) => [team.team_id, team]));
  const canManageOrganization = organization?.role === "owner" || organization?.role === "admin";
  const canManageAnyTeam = organization?.teams.some((team) => ["owner", "admin"].includes(teamMemberships.get(team.id)?.role ?? "")) ?? false;
  const logoMutation = useMutation({ mutationFn: (file: File) => uploadOrganizationLogo(organization!.id, file), onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["navigation"] }) });

  if (!organization) return <SettingsShell><p className="text-sm text-muted-foreground">Loading workspace…</p></SettingsShell>;
  return <SettingsShell><header className="mb-8"><p className="text-xs text-muted-foreground">Workspace</p><h1 className="mt-2 text-2xl font-medium tracking-tight">{organization.name}</h1><p className="mt-2 text-sm text-muted-foreground">Workspace identity and team structure.</p></header><div className="grid max-w-2xl gap-8">{canManageOrganization ? <section className="grid gap-4 rounded-lg border border-border/70 bg-card/20 p-5"><div><h2 className="text-sm font-medium">Workspace image</h2><p className="mt-1 text-xs text-muted-foreground">Shown throughout the workspace.</p></div><div className="flex items-center gap-3">{organization.logo_url ? <img src={organization.logo_url} alt="" className="size-12 rounded-lg border border-border/60 object-cover" /> : <div className="grid size-12 place-items-center rounded-lg border border-border/60 text-sm font-semibold">{organization.name.slice(0, 1).toUpperCase()}</div>}<Button className="ml-auto" variant="outline" size="sm" onClick={() => organizationLogoInput.current?.click()} disabled={logoMutation.isPending}>{logoMutation.isPending ? "Uploading…" : "Change image"}</Button><input ref={organizationLogoInput} type="file" accept="image/png,image/svg+xml" className="hidden" onChange={(event) => { const file = event.target.files?.[0]; if (file) logoMutation.mutate(file); event.target.value = ""; }} /></div>{logoMutation.error ? <p role="alert" className="text-xs text-destructive">Could not update the workspace image: {logoMutation.error.message}</p> : null}</section> : null}<section className="grid gap-3"><div><h2 className="text-sm font-medium">Teams</h2><p className="mt-1 text-xs text-muted-foreground">Teams organize issue ownership and project access.</p></div>{organization.teams.map((team) => <div key={team.id} className="flex items-center justify-between gap-4 rounded-lg border border-border/70 bg-card/20 px-4 py-3 text-xs"><div className="flex min-w-0 items-center gap-2"><TeamEmojiEditor team={team} canManage={["owner", "admin"].includes(teamMemberships.get(team.id)?.role ?? "")} onSaved={() => void queryClient.invalidateQueries({ queryKey: ["navigation"] })} /><span className="font-medium">{team.name}</span><span className="text-muted-foreground">{team.key} · {team.projects.length} project{team.projects.length === 1 ? "" : "s"}</span></div><span className="shrink-0 text-muted-foreground">{teamMemberships.get(team.id)?.role ?? "member"}</span></div>)}</section>{!canManageOrganization && !canManageAnyTeam ? <p className="text-xs text-muted-foreground">You can view workspace structure, but only workspace and team admins can change it.</p> : null}</div></SettingsShell>;
}
