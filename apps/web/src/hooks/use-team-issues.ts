import { useMemo } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { useQuery } from "@tanstack/react-query";

import { getTeamIssues } from "@/lib/api";
import { createHumanIssueCollection, issueFromSyncRecord } from "@/lib/metadata-sync";

export function useTeamIssues(teamId: string) {
  const query = useQuery({
    queryKey: ["issues", "team", teamId],
    queryFn: () => getTeamIssues(teamId),
    enabled: Boolean(teamId),
  });
  const collection = useMemo(() => createHumanIssueCollection(), []);
  const replicated = useLiveQuery(() => collection, [collection]);
  const teamIssues = replicated?.data
    ?.map(issueFromSyncRecord)
    .filter((issue) => issue.team_id === teamId);

  return { ...query, data: teamIssues ?? query.data };
}
