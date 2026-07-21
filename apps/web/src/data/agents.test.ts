import { describe, expect, it } from "vitest";

import { agentCliCommand } from "./agents";

describe("agent setup helpers", () => {
  it("creates a copyable CLI handoff without changing the token", () => {
    expect(agentCliCommand("project-1", {
      session_id: "session-1",
      agent_token: "token-1",
      expires_at: "2026-07-21T12:00:00Z",
    })).toBe("RIICHI_PROJECT_ID=project-1 RIICHI_SESSION_ID=session-1 RIICHI_AGENT_TOKEN=token-1 riichi-agent ready");
  });
});
