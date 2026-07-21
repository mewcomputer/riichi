import type { HumanQueueIssue } from "@/lib/api";
import type { QueueItem, QueueState } from "@/data/queue";
import type { IssueImportance } from "@/components/issues/issue-importance-menu";
import type { IssueStatus } from "@/components/issues/issue-status-menu";

export type QueueFilter = "all" | QueueState;
export type QueueView = "all" | "active" | "backlog" | "my_work";
export type QueueAdvancedFilter = {
  status: HumanQueueIssue["status"] | "all";
  importance: HumanQueueIssue["importance"] | "all";
  teamKey: string;
  projectId: string;
  assignee: "all" | "me" | "assigned" | "unassigned";
  label: string;
};

export type QueueIssueClaimHandler = (item: QueueItem) => void;

export type QueueMutationFeedback = {
  state: "pending" | "confirmed" | "rejected";
  message?: string;
};

export type QueueBulkAction =
  | { kind: "status"; value: IssueStatus }
  | { kind: "importance"; value: IssueImportance }
  | { kind: "label"; value: string }
  | { kind: "assignee"; value: string };

export const stateCopy: Record<QueueState, string> = {
  ready: "Ready",
  attention: "Attention",
  held: "On hold",
};
