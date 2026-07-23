import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useParams } from "@tanstack/react-router";

import { getCurrentUser } from "@/lib/api";
import { SettingsShell } from "@/components/settings/settings-shell";
import { TeamEmojiEditor } from "@/components/team/team-emoji-editor";
import { TeamMark } from "@/components/team/team-mark";
import { useNavigation } from "../hooks/use-navigation";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";

export function TeamSettingsPage() {
  const { organizationSlug, teamKey } = useParams({ from: "/$organizationSlug/teams/$teamKey/settings" });
  const queryClient = useQueryClient();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const organization = navigationQuery.data?.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
  const team = organization?.teams.find((candidate) => candidate.key.toLowerCase() === teamKey.toLowerCase());
  const teamMembership = meQuery.data?.teams.find((candidate) => candidate.team_id === team?.id);
  const canManage = teamMembership?.role === "owner" || teamMembership?.role === "admin";

  if (!team) return <div className="p-8 text-sm text-muted-foreground">Loading team…</div>;
  return <SettingsShell>
        <header className="mb-8"><p className="text-xs text-muted-foreground">Team settings</p><h1 className="mt-2 text-2xl font-medium tracking-tight">{team.name}</h1><p className="mt-2 text-sm text-muted-foreground">Identity and access for {team.name}.</p></header>
        <section className="grid max-w-2xl gap-4 rounded-lg border border-border/70 bg-card/20 p-5">
          <div><h2 className="text-sm font-medium">Identity</h2><p className="mt-1 text-xs text-muted-foreground">This emoji appears beside the team throughout Riichi.</p></div>
          <div className="flex items-center justify-between rounded-md border border-border/60 px-3 py-3"><div className="flex items-center gap-3"><TeamMark value={team.emoji} className="size-5" /><div className="grid gap-1"><span className="text-sm font-medium">{team.name}</span><span className="font-mono text-xs text-muted-foreground">{team.key}</span></div></div><TeamEmojiEditor team={team} canManage={canManage} onSaved={() => void queryClient.invalidateQueries({ queryKey: ["navigation"] })} /></div>
        </section>
      </SettingsShell>;
}
