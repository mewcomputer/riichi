import { useParams } from "@tanstack/react-router";

import { QueuePage } from "./queue";
import { useNavigation } from "../hooks/use-navigation";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";

export function TeamIssuesPage() {
  const { organizationSlug, teamKey } = useParams({ from: "/$organizationSlug/teams/$teamKey/issues" });
  const navigationQuery = useNavigation();
  const organization = navigationQuery.data?.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
  const team = organization?.teams.find((candidate) => candidate.key.toLowerCase() === teamKey.toLowerCase());

  if (navigationQuery.isPending) return <div className="p-8 text-sm text-muted-foreground">Loading team…</div>;
  if (!team) return <div className="p-8 text-sm text-destructive">That team could not be found.</div>;
  return <QueuePage teamId={team.id} organizationSlug={organizationSlug} />;
}
