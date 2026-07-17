import { useQuery } from "@tanstack/react-query";

import { getPendingApprovals } from "@/lib/api";

export function usePendingApprovals() {
  return useQuery({ queryKey: ["approvals", "pending"], queryFn: getPendingApprovals });
}
