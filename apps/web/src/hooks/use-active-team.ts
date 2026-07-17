import { useCallback, useEffect, useState } from "react";

import type { HumanMe } from "@/lib/api";

const STORAGE_KEY = "riichi.activeTeamId";

export function resolveActiveTeam(teams: HumanMe["teams"] | undefined, selectedId: string | undefined) {
  return teams?.find((team) => team.team_id === selectedId) ?? teams?.[0];
}

function storedTeamId() {
  if (typeof window === "undefined") return undefined;
  return window.localStorage.getItem(STORAGE_KEY) || undefined;
}

export function useActiveTeam(teams: HumanMe["teams"] | undefined) {
  const [selectedId, setSelectedId] = useState<string | undefined>(storedTeamId);
  const activeTeam = resolveActiveTeam(teams, selectedId);

  useEffect(() => {
    if (activeTeam) window.localStorage.setItem(STORAGE_KEY, activeTeam.team_id);
  }, [activeTeam]);

  const selectTeam = useCallback((teamId: string) => {
    setSelectedId(teamId);
    window.localStorage.setItem(STORAGE_KEY, teamId);
  }, []);

  return { activeTeam, selectTeam };
}
