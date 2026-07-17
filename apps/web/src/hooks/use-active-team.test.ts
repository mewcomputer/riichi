import { describe, expect, it } from "vitest";

import { resolveActiveTeam } from "./use-active-team";

const teams = [
  { team_id: "team-a", team_name: "Alpha", team_key: "ALP", role: "member" as const },
  { team_id: "team-b", team_name: "Beta", team_key: "BET", role: "admin" as const },
];

describe("resolveActiveTeam", () => {
  it("preserves a selected authorized team", () => {
    expect(resolveActiveTeam(teams, "team-b")?.team_name).toBe("Beta");
  });

  it("falls back to the first team when selection is missing or inaccessible", () => {
    expect(resolveActiveTeam(teams, "unknown")?.team_id).toBe("team-a");
    expect(resolveActiveTeam(undefined, "team-a")).toBeUndefined();
  });
});
