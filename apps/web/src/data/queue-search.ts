import type { QueueAdvancedFilter, QueueFilter, QueueView } from "@/components/queue/types";

export type QueueSearchState = {
  filter: QueueFilter;
  view: QueueView;
  query: string;
  showDetails: boolean;
  advancedFilter: QueueAdvancedFilter;
};

const queueFilters = new Set<QueueFilter>(["all", "ready", "attention", "held"]);
const queueViews = new Set<QueueView>(["all", "active", "backlog"]);
const queueStatuses = new Set<QueueAdvancedFilter["status"]>([
  "all",
  "triage",
  "todo",
  "in_progress",
  "blocked",
  "done",
  "canceled",
]);
const queueImportances = new Set<QueueAdvancedFilter["importance"]>([
  "all",
  "none",
  "low",
  "medium",
  "high",
  "urgent",
]);

function valueOr<T>(value: unknown, values: Set<T>, fallback: T): T {
  return typeof value === "string" && values.has(value as T) ? value as T : fallback;
}

export function parseQueueSearch(search: Record<string, unknown>): QueueSearchState {
  return {
    filter: valueOr(search.filter, queueFilters, "all"),
    view: valueOr(search.view, queueViews, "all"),
    query: typeof search.q === "string" ? search.q : "",
    showDetails: search.details !== "0",
    advancedFilter: {
      status: valueOr(search.status, queueStatuses, "all"),
      importance: valueOr(search.importance, queueImportances, "all"),
      teamKey: typeof search.team === "string" && search.team ? search.team : "all",
      projectId: typeof search.project === "string" && search.project ? search.project : "all",
    },
  };
}

export function serializeQueueSearch(state: QueueSearchState) {
  return {
    ...(state.filter !== "all" ? { filter: state.filter } : {}),
    ...(state.view !== "all" ? { view: state.view } : {}),
    ...(state.query ? { q: state.query } : {}),
    ...(!state.showDetails ? { details: "0" } : {}),
    ...(state.advancedFilter.status !== "all" ? { status: state.advancedFilter.status } : {}),
    ...(state.advancedFilter.importance !== "all" ? { importance: state.advancedFilter.importance } : {}),
    ...(state.advancedFilter.teamKey !== "all" ? { team: state.advancedFilter.teamKey } : {}),
    ...(state.advancedFilter.projectId !== "all" ? { project: state.advancedFilter.projectId } : {}),
  };
}

export function moveQueueSelection(issueIds: string[], selectedIssueId: string | null, direction: 1 | -1) {
  if (issueIds.length === 0) return null;
  const currentIndex = issueIds.indexOf(selectedIssueId ?? "");
  const startIndex = currentIndex < 0 ? (direction === 1 ? 0 : issueIds.length - 1) : currentIndex + direction;
  return issueIds[Math.min(issueIds.length - 1, Math.max(0, startIndex))] ?? null;
}
