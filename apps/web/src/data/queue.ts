import type { HumanQueueIssue } from "@/lib/api";
import type { QueueAdvancedFilter, QueueView } from "@/components/queue/types";
import { issueStatuses } from "@/components/issues/issue-status-menu";

export type QueueState = "ready" | "attention" | "held";

export type QueueItem = {
  teamKey: string;
  teamName: string;
  projectId: string;
  projectName: string;
  id: string;
  issueId: string;
  title: string;
  description: string;
  state: QueueState;
  status: HumanQueueIssue["status"];
  importance: HumanQueueIssue["importance"];
  age: string;
  reason: string;
  activeLeaseId: string | null;
  leaseExpiresAt: string | null;
  assigneeAccountId: string | null;
  labels: string[];
};

export function groupQueueItemsByStatus(items: QueueItem[]) {
  const byStatus = new Map<QueueItem["status"], QueueItem[]>();
  for (const item of items) {
    const group = byStatus.get(item.status);
    if (group) group.push(item);
    else byStatus.set(item.status, [item]);
  }

  return issueStatuses
    .map((status) => ({ ...status, items: byStatus.get(status.value) ?? [] }))
    .filter((group) => group.items.length > 0);
}

export function deriveQueueState(issue: HumanQueueIssue): QueueState {
  if (
    issue.active_hold_count > 0 ||
    issue.unresolved_blocker_count > 0 ||
    issue.status === "triage" ||
    issue.status === "blocked"
  ) {
    return "held";
  }

  if (
    issue.status === "todo" &&
    issue.unresolved_blocker_count === 0 &&
    issue.active_lease_id === null
  ) {
    return "ready";
  }

  return "attention";
}

export function queueReason(issue: HumanQueueIssue) {
  if (issue.active_hold_count > 0) {
    return `${issue.active_hold_count} active hold${issue.active_hold_count === 1 ? "" : "s"}`;
  }
  if (issue.unresolved_blocker_count > 0) {
    return `${issue.unresolved_blocker_count} unresolved blocker${issue.unresolved_blocker_count === 1 ? "" : "s"}`;
  }
  if (issue.active_lease_id) return "Leased to an agent";
  if (issue.specification_changed_since_review) return "Specification changed since review";
  if (!issue.spec_complete) return "Spec needed";
  if (!issue.agent_eligible) return "Human-owned issue";
  if (issue.status === "done") return "Completed";
  if (issue.status === "canceled") return "Canceled";
  return "Ready for dispatch";
}

export function matchesQueueAdvancedFilter(item: QueueItem, filter: QueueAdvancedFilter, accountId?: string) {
  return (
    (filter.status === "all" || item.status === filter.status) &&
    (filter.importance === "all" || item.importance === filter.importance) &&
    (filter.teamKey === "all" || item.teamKey === filter.teamKey) &&
    (filter.projectId === "all" || item.projectId === filter.projectId) &&
    (filter.assignee === "all" ||
      (filter.assignee === "assigned" && item.assigneeAccountId !== null) ||
      (filter.assignee === "unassigned" && item.assigneeAccountId === null) ||
      (filter.assignee === "me" && item.assigneeAccountId === accountId)) &&
    (filter.label === "all" || item.labels.includes(filter.label))
  );
}

export function matchesQueueView(item: QueueItem, view: QueueView, accountId?: string) {
  if (view === "all") return true;
  if (view === "active") return item.state !== "held";
  if (view === "backlog") return item.state === "held";
  return item.assigneeAccountId !== null && item.assigneeAccountId === accountId;
}

export function addQueueLabel(labels: string[], label: string) {
  return [...new Set([...labels, label])];
}

export function formatQueueAge(createdAt: string, now = new Date()) {
  const elapsedMinutes = Math.max(
    0,
    Math.floor((now.getTime() - new Date(createdAt).getTime()) / 60_000),
  );
  if (elapsedMinutes < 60) return `${elapsedMinutes}m`;
  const hours = Math.floor(elapsedMinutes / 60);
  if (hours < 24) return `${hours}h ${elapsedMinutes % 60}m`;
  return `${Math.floor(hours / 24)}d`;
}

export function toQueueItem(issue: HumanQueueIssue, now = new Date()): QueueItem {
  return {
    teamKey: issue.team_key,
    teamName: issue.team_name,
    projectId: issue.project_id,
    projectName: issue.project_name,
    id: issue.display_key,
    issueId: issue.id,
    title: issue.title,
    description: issue.body,
    state: deriveQueueState(issue),
    status: issue.status,
    importance: issue.importance,
    age: formatQueueAge(issue.created_at, now),
    reason: queueReason(issue),
    activeLeaseId: issue.active_lease_id,
    leaseExpiresAt: issue.lease_expires_at,
    assigneeAccountId: issue.assignee_account_id,
    labels: issue.labels,
  };
}
