import type { HumanQueueIssue } from "@/lib/api";
import type { QueueItem, QueueState } from "@/data/queue";

export type QueueFilter = "all" | QueueState;
export type QueueView = "all" | "active" | "backlog";
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

export const stateCopy: Record<QueueState, string> = {
  ready: "Ready",
  attention: "Attention",
  held: "On hold",
};
