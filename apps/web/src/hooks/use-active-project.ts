import { useCallback, useEffect, useState } from "react";

import type { HumanMembership } from "@/lib/api";

const STORAGE_KEY = "riichi.activeProjectId";

function configuredProjectId() {
  return import.meta.env.VITE_RIICHI_PROJECT_ID || undefined;
}

function storedProjectId() {
  if (typeof window === "undefined") return undefined;
  return window.localStorage.getItem(STORAGE_KEY) || undefined;
}

export function useActiveProject(memberships: HumanMembership[] | undefined) {
  const [selectedId, setSelectedId] = useState<string | undefined>(() => configuredProjectId() ?? storedProjectId());
  const availableId = memberships?.some((membership) => membership.project_id === selectedId)
    ? selectedId
    : memberships?.[0]?.project_id;
  const activeMembership = memberships?.find((membership) => membership.project_id === availableId);

  useEffect(() => {
    if (!activeMembership || configuredProjectId()) return;
    window.localStorage.setItem(STORAGE_KEY, activeMembership.project_id);
  }, [activeMembership]);

  const selectProject = useCallback((projectId: string) => {
    if (configuredProjectId()) return;
    setSelectedId(projectId);
    window.localStorage.setItem(STORAGE_KEY, projectId);
  }, []);

  return { activeMembership, projectId: availableId, selectProject };
}
