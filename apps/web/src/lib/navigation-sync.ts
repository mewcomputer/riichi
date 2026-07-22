import { createCollection } from "@tanstack/react-db";
import { electricCollectionOptions } from "@tanstack/electric-db-collection";

import type { NavigationResponse } from "./api";
import { authenticatedShapeUrl } from "./shape-url";
import { registerSessionCollection } from "./session-state";

export type NavigationSyncRecord = {
  account_id: string;
  organization_id: string;
  organization_name: string;
  organization_role: string;
  organization_has_logo: boolean;
  team_id: string;
  team_name: string;
  team_key: string;
  team_emoji: string | null;
  project_id: string;
  project_name: string;
  project_role: string;
};

export function createNavigationCollection() {
  if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED !== "true") return null;

  return registerSessionCollection(createCollection(
    electricCollectionOptions<NavigationSyncRecord>({
      id: "riichi-navigation",
      getKey: (row) => `${row.account_id}:${row.team_id}:${row.project_id}`,
      shapeOptions: {
        url: authenticatedShapeUrl("/api/v1/sync/navigation"),
      },
    }),
  ));
}

export function navigationFromSyncRows(rows: NavigationSyncRecord[]): NavigationResponse {
  const organizations: NavigationResponse["organizations"] = [];
  const organizationById = new Map<string, NavigationResponse["organizations"][number]>();
  const teamByOrganization = new Map<string, NavigationResponse["organizations"][number]["teams"][number]>();

  for (const row of rows) {
    let organization = organizationById.get(row.organization_id);
    if (!organization) {
      organization = {
        id: row.organization_id,
        name: row.organization_name,
        role: row.organization_role,
        logo_url: row.organization_has_logo ? `/api/v1/organizations/${row.organization_id}/logo` : null,
        teams: [],
      };
      organizationById.set(row.organization_id, organization);
      organizations.push(organization);
    }

    const teamKey = `${row.organization_id}:${row.team_id}`;
    let team = teamByOrganization.get(teamKey);
    if (!team) {
      team = {
        id: row.team_id,
        name: row.team_name,
        key: row.team_key,
        emoji: row.team_emoji,
        projects: [],
        views: [],
      };
      teamByOrganization.set(teamKey, team);
      organization.teams.push(team);
    }
  team.projects.push({ id: row.project_id, name: row.project_name, icon: null, role: row.project_role });
  }

  organizations.sort((left, right) => left.name.localeCompare(right.name));
  for (const organization of organizations) {
    organization.teams.sort((left, right) => left.name.localeCompare(right.name));
    for (const team of organization.teams) {
      team.projects.sort((left, right) => left.name.localeCompare(right.name));
    }
  }
  return { organizations };
}
