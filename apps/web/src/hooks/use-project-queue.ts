import { useQuery } from "@tanstack/react-query";

import { getProjectQueue } from "@/lib/api";

export function useProjectQueue(projectId: string | undefined) {
  return useQuery({
    queryKey: ["project", projectId, "queue"],
    queryFn: () => getProjectQueue(projectId!),
    enabled: Boolean(projectId),
  });
}
