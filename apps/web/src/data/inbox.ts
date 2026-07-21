import type { Notification } from "@/lib/api";

export function filterNotifications(notifications: Notification[], projectId: string) {
  return projectId === "all" ? notifications : notifications.filter((notification) => notification.project_id === projectId);
}

export function notificationTitle(kind: Notification["kind"]) {
  return {
    comment: "New comment",
    approval: "Approval requested",
    assignment: "Assignment",
    invitation: "Invitation",
    takeover: "Takeover",
    lease: "Lease update",
  }[kind];
}

export function notificationSummary(notification: Notification) {
  const body = notification.payload.body;
  if (typeof body === "string" && body.trim()) return body;
  if (notification.kind === "approval" && typeof notification.payload.target_version === "number") {
    return `Review the proposed change against version ${notification.payload.target_version}.`;
  }
  if (typeof notification.payload.reason === "string" && notification.payload.reason.trim()) return notification.payload.reason;
  return "You have a new Riichi notification.";
}

export function notificationAction(notification: Notification) {
  if (notification.kind === "approval") return "Review approval";
  if (notification.issue_id) return "Open issue";
  return null;
}
