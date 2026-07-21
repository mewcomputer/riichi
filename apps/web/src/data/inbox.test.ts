import { describe, expect, it } from "vitest";

import type { Notification } from "@/lib/api";
import { filterNotifications, notificationSummary, notificationTitle } from "./inbox";

const notification = (overrides: Partial<Notification> = {}): Notification => ({
  id: "notification-1",
  recipient_account_id: "account-1",
  kind: "comment",
  project_id: "project-1",
  issue_id: "issue-1",
  actor_id: null,
  payload: { body: "A comment" },
  created_at: "2026-07-21T12:00:00Z",
  read_at: null,
  ...overrides,
});

describe("inbox helpers", () => {
  it("filters notifications by project while keeping global mode intact", () => {
    const items = [notification(), notification({ id: "notification-2", project_id: "project-2" }), notification({ id: "notification-3", project_id: null })];
    expect(filterNotifications(items, "all")).toHaveLength(3);
    expect(filterNotifications(items, "project-2").map((item) => item.id)).toEqual(["notification-2"]);
  });

  it("uses typed notification copy before falling back to generic copy", () => {
    expect(notificationTitle("approval")).toBe("Approval requested");
    expect(notificationSummary(notification({ kind: "approval", payload: { target_version: 4, approval_id: "approval-1" }, issue_id: "issue-1" }))).toContain("version 4");
    expect(notificationSummary(notification({ payload: {} }))).toBe("You have a new Riichi notification.");
  });
});

