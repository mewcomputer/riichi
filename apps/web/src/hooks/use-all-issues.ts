import { useMemo } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { useQuery } from "@tanstack/react-query";

import { getAllIssues } from "@/lib/api";
import { createHumanIssueCollection, issueFromSyncRecord } from "@/lib/metadata-sync";

export function useAllIssues() {
  const query = useQuery({
    queryKey: ["issues", "all"],
    queryFn: () => getAllIssues(),
  });
  const collection = useMemo(() => createHumanIssueCollection(), []);
  const replicated = useLiveQuery(() => collection, [collection]);

  return { ...query, data: replicated?.data?.map(issueFromSyncRecord) ?? query.data };
}
