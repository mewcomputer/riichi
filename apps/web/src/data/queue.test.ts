import { describe, expect, it } from "vitest";

import type { HumanQueueIssue } from "@/lib/api";
import { deriveQueueState, formatQueueAge, groupQueueItemsByStatus, matchesQueueAdvancedFilter, queueReason, toQueueItem } from "./queue";

const baseIssue: HumanQueueIssue = {
  team_id: "team-id",
  team_name: "Riichi",
  team_key: "RII",
  project_id: "project-id",
  project_name: "Pilot project",
  id: "issue-id",
  display_key: "RII-1",
  title: "Make the queue real",
  body: "Use the authoritative API.",
  status: "todo",
  importance: "none",
  agent_eligible: true,
  spec_complete: true,
  specification_changed_since_review: false,
  unresolved_blocker_count: 0,
  active_hold_count: 0,
  active_lease_id: null,
  lease_expires_at: null,
  created_at: "2026-07-11T12:00:00.000Z",
  updated_at: "2026-07-11T12:00:00.000Z",
  rank: 0,
  dispatch_version: 1,
  assignee_account_id: null,
  labels: [],
};

describe("queue state mapping", () => {
  it("marks an eligible unclaimed todo issue as ready", () => {
    expect(deriveQueueState(baseIssue)).toBe("ready");
  });

  it("holds work only for actual blockers, holds, or triage states", () => {
    expect(deriveQueueState({ ...baseIssue, unresolved_blocker_count: 1 })).toBe("held");
    expect(deriveQueueState({ ...baseIssue, active_hold_count: 1 })).toBe("held");
    expect(deriveQueueState({ ...baseIssue, status: "triage" })).toBe("held");
  });

  it("keeps human-owned issues ready without requiring agent dispatch flags", () => {
    const issue = { ...baseIssue, agent_eligible: false, spec_complete: false };
    expect(deriveQueueState(issue)).toBe("ready");
    expect(queueReason(issue)).toBe("Spec needed");
  });

  it("surfaces a changed specification as a review requirement", () => {
    const issue = { ...baseIssue, specification_changed_since_review: true };
    expect(queueReason(issue)).toBe("Specification changed since review");
  });

  it("keeps leased work visible as attention instead of pretending it is ready", () => {
    const issue = { ...baseIssue, active_lease_id: "lease-id", status: "in_progress" as const };
    expect(deriveQueueState(issue)).toBe("attention");
    expect(queueReason(issue)).toBe("Leased to an agent");
  });

  it("maps server fields without inventing priority or labels", () => {
    const item = toQueueItem(baseIssue, new Date("2026-07-11T12:42:00.000Z"));
    expect(item).toMatchObject({
      id: "RII-1",
      title: "Make the queue real",
      description: "Use the authoritative API.",
      age: "42m",
      state: "ready",
    });
  });

  it("clamps future timestamps to zero age", () => {
    expect(formatQueueAge("2026-07-11T13:00:00.000Z", new Date("2026-07-11T12:00:00.000Z"))).toBe("0m");
  });

  it("matches each advanced filter criterion independently", () => {
    const item = toQueueItem({ ...baseIssue, importance: "high" }, new Date("2026-07-11T12:42:00.000Z"));
    const baseFilter = { assignee: "all" as const, label: "all" };
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, status: "all", importance: "all", teamKey: "all", projectId: "all" })).toBe(true);
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, status: "todo", importance: "high", teamKey: "RII", projectId: "project-id" })).toBe(true);
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, status: "blocked", importance: "high", teamKey: "RII", projectId: "project-id" })).toBe(false);
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, status: "todo", importance: "urgent", teamKey: "RII", projectId: "project-id" })).toBe(false);
  });

  it("matches assignee and label filters against preserved server fields", () => {
    const item = toQueueItem({ ...baseIssue, assignee_account_id: "account-1", labels: ["customer"] });
    const baseFilter = { status: "all" as const, importance: "all" as const, teamKey: "all", projectId: "all", label: "all" };
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, assignee: "assigned" })).toBe(true);
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, assignee: "me" }, "account-1")).toBe(true);
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, assignee: "me" }, "account-2")).toBe(false);
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, assignee: "unassigned" })).toBe(false);
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, assignee: "all", label: "customer" })).toBe(true);
    expect(matchesQueueAdvancedFilter(item, { ...baseFilter, assignee: "all", label: "billing" })).toBe(false);
  });

  it("keeps issues in separate lifecycle status groups", () => {
    const done = toQueueItem({ ...baseIssue, id: "done-id", display_key: "RII-2", status: "done" }, new Date("2026-07-11T12:42:00.000Z"));
    const groups = groupQueueItemsByStatus([toQueueItem(baseIssue), done]);

    expect(groups.map((group) => group.value)).toEqual(["todo", "done"]);
    expect(groups.find((group) => group.value === "todo")?.items.map((item) => item.issueId)).toEqual(["issue-id"]);
    expect(groups.find((group) => group.value === "done")?.items.map((item) => item.issueId)).toEqual(["done-id"]);
  });
});
