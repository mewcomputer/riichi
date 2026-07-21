import type { QueueAdvancedFilter, QueueFilter, QueueView } from "@/components/queue/types";

export type QueueSearchState = {
  filter: QueueFilter;
  view: QueueView;
  query: string;
  showDetails: boolean;
  advancedFilter: QueueAdvancedFilter;
};

export type QueueFilterChip = {
  id: string;
  label: string;
  clear: Partial<QueueSearchState>;
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

function readable(value: string) {
  return value.replaceAll("_", " ").replace(/\b\w/g, (character) => character.toUpperCase());
}

export function activeQueueFilterChips(
  state: QueueSearchState,
  labels: { team?: string; project?: string } = {},
): QueueFilterChip[] {
  const chips: QueueFilterChip[] = [];
  if (state.filter !== "all") chips.push({ id: "filter", label: readable(state.filter), clear: { filter: "all" } });
  if (state.view !== "all") chips.push({ id: "view", label: readable(state.view), clear: { view: "all" } });
  if (state.query) chips.push({ id: "query", label: `Search: ${state.query}`, clear: { query: "" } });
  if (state.advancedFilter.status !== "all") chips.push({ id: "status", label: `Status: ${readable(state.advancedFilter.status)}`, clear: { advancedFilter: { ...state.advancedFilter, status: "all" } } });
  if (state.advancedFilter.importance !== "all") chips.push({ id: "importance", label: `Priority: ${readable(state.advancedFilter.importance)}`, clear: { advancedFilter: { ...state.advancedFilter, importance: "all" } } });
  if (state.advancedFilter.teamKey !== "all") chips.push({ id: "team", label: `Team: ${labels.team ?? state.advancedFilter.teamKey}`, clear: { advancedFilter: { ...state.advancedFilter, teamKey: "all" } } });
  if (state.advancedFilter.projectId !== "all") chips.push({ id: "project", label: `Project: ${labels.project ?? state.advancedFilter.projectId}`, clear: { advancedFilter: { ...state.advancedFilter, projectId: "all" } } });
  return chips;
}
