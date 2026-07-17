import { useQuery } from "@tanstack/react-query";

import { getIssueActivity } from "@/lib/api";

export function useIssueActivity(projectId: string | undefined, issueId: string) {
  return useQuery({
    queryKey: ["issue", projectId, issueId, "activity"],
    queryFn: () => getIssueActivity(projectId!, issueId),
    enabled: Boolean(projectId),
  });
}
