import { useMemo } from "react";
import { useLiveQuery } from "@tanstack/react-db";

import { createHumanAgentCollection, type HumanAgentSyncRecord } from "@/lib/metadata-sync";
import type { AgentRoster } from "@/lib/api";

export function useHumanAgentRoster(teamId: string | undefined): AgentRoster | undefined {
  const collection = useMemo(
    () => (teamId ? createHumanAgentCollection(teamId) : null),
    [teamId],
  );
  const replicated = useLiveQuery(() => collection, [collection]);
  const records = replicated.data as HumanAgentSyncRecord[] | undefined;
  if (!records) return undefined;

  return {
    roles: records.map((record) => ({
      id: record.agent_role_id,
      project_id: record.project_id,
      team_id: record.team_id,
      display_name: record.display_name,
      owner_account_id: record.owner_account_id,
      capabilities: record.capabilities,
      revoked_at: record.revoked_at,
      active_session_count: record.active_session_count,
    })),
    sessions: records.flatMap((record) => record.sessions),
  };
}
