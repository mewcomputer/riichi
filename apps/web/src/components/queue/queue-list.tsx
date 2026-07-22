import { Circle, CircleAlert, CircleCheck, CircleDot, CircleX, Search } from "lucide-react";
import { Link } from "@tanstack/react-router";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { IssueStatusMenu, type IssueStatus } from "@/components/issues/issue-status-menu";
import { IssueImportanceMenu, type IssueImportance } from "@/components/issues/issue-importance-menu";
import { groupQueueItemsByStatus, type QueueItem } from "@/data/queue";
import type { QueueMutationFeedback } from "./types";

function StatusMark({ status }: { status: IssueStatus }) {
  if (status === "triage" || status === "blocked") return <CircleAlert className="size-4 text-orange-400" />;
  if (status === "in_progress") return <CircleDot className="size-4 text-blue-400" />;
  if (status === "done") return <CircleCheck className="size-4 text-emerald-400" />;
  if (status === "canceled") return <CircleX className="size-4 text-muted-foreground" />;
  return <Circle className="size-4 text-muted-foreground" />;
}

function QueueRow({ item, organizationSlug, selected, selectedForBulk, feedback, showDetails, onOpenIssue, onToggleSelection, onStatusChange, onImportanceChange }: { item: QueueItem; organizationSlug: string; selected: boolean; selectedForBulk: boolean; feedback?: QueueMutationFeedback; showDetails: boolean; onOpenIssue: (item: QueueItem) => void; onToggleSelection: (item: QueueItem, checked: boolean) => void; onStatusChange: (item: QueueItem, status: IssueStatus) => void; onImportanceChange: (item: QueueItem, importance: IssueImportance) => void }) {
  return (
    <div data-queue-item-id={item.issueId} className={`grid min-h-11 grid-cols-[24px_24px_24px_minmax(0,1fr)_auto] items-center gap-2 border-b border-border/40 px-3 text-xs transition-colors hover:bg-muted/35 sm:min-h-10 sm:grid-cols-[24px_24px_64px_24px_minmax(220px,1fr)_auto] sm:px-4 ${selected ? "bg-muted/45 ring-1 ring-inset ring-ring/60" : ""}`}>
      <Checkbox checked={selectedForBulk} onCheckedChange={(checked) => onToggleSelection(item, checked === true)} aria-label={`Select ${item.id}`} />
      <IssueImportanceMenu importance={item.importance} compact className="size-11 sm:size-7" onChange={(importance) => onImportanceChange(item, importance)} />
      <span className="hidden truncate font-mono text-[11px] text-muted-foreground sm:block">{item.id}</span>
        <IssueStatusMenu
          status={item.status}
          icon={<StatusMark status={item.status} />}
          className="size-11 sm:size-7"
        onChange={(status) => onStatusChange(item, status)}
      />
      <Link
        to="/$organizationSlug/teams/$teamKey/issues/$issueId"
        params={{ organizationSlug, teamKey: item.teamKey, issueId: item.issueId }}
        className="contents"
        aria-current={selected ? "true" : undefined}
        onClick={(event) => {
          event.preventDefault();
          onOpenIssue(item);
        }}
      >
        <div className="flex min-w-0 items-center gap-2">
          <span className="shrink-0 font-mono text-[10px] text-muted-foreground sm:hidden">{item.id}</span>
          <span className="truncate text-[13px] text-foreground/90">{item.title}</span>
          <span className="hidden truncate text-[11px] text-muted-foreground lg:inline">· {item.projectName}</span>
          {showDetails && item.reason !== "Ready for dispatch" ? <Badge variant="outline" className="hidden h-5 max-w-36 px-1.5 text-[10px] text-muted-foreground xl:inline-flex">{item.reason}</Badge> : null}
        </div>
        <div className="flex items-center justify-end gap-2 text-[10px] text-muted-foreground">
          {feedback ? <span role={feedback.state === "rejected" ? "alert" : "status"} className={feedback.state === "rejected" ? "text-destructive" : feedback.state === "pending" ? "text-foreground/60" : "text-emerald-400"}>{feedback.state === "pending" ? "Saving…" : feedback.state === "confirmed" ? "Saved" : feedback.message ?? "Update failed"}</span> : null}
          <span>{item.age.replace(" in queue", "")}</span>
        </div>
      </Link>
    </div>
  );
}

export function QueueList({
  organizationSlug,
  items,
  selectedIssueId = null,
  feedbackByIssueId = {},
  showDetails,
  loading = false,
  error,
  onRetry,
  authRequired = false,
  onOpenIssue,
  onStatusChange,
  onImportanceChange,
  stale = false,
  selectedIssueIds = new Set(),
  onToggleSelection,
}: {
  organizationSlug: string;
  items: QueueItem[];
  selectedIssueId?: string | null;
  feedbackByIssueId?: Record<string, QueueMutationFeedback>;
  showDetails: boolean;
  loading?: boolean;
  error?: Error;
  onRetry?: () => void;
  authRequired?: boolean;
  onOpenIssue: (item: QueueItem) => void;
  onStatusChange: (item: QueueItem, status: IssueStatus) => void;
  onImportanceChange: (item: QueueItem, importance: IssueImportance) => void;
  stale?: boolean;
  selectedIssueIds?: Set<string>;
  onToggleSelection: (item: QueueItem, checked: boolean) => void;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <div className="min-w-0">
        {stale ? <div role="status" className="flex items-center justify-between gap-3 border-b border-orange-400/30 bg-orange-400/10 px-4 py-2 text-xs text-orange-200"><span>Showing the last server snapshot. Refresh failed.</span>{onRetry ? <Button variant="ghost" size="sm" className="h-7" onClick={onRetry}>Retry</Button> : null}</div> : null}
        {loading ? (
          <div className="space-y-2 p-4">
            {Array.from({ length: 5 }, (_, index) => (
              <Skeleton key={index} className="h-10 w-full" />
            ))}
          </div>
        ) : error ? (
          <Empty className="min-h-56 rounded-none border-0 p-0">
            <EmptyHeader>
              <EmptyMedia variant="icon"><Search /></EmptyMedia>
              <EmptyTitle>{error.message}</EmptyTitle>
            </EmptyHeader>
            {authRequired ? (
              <Button variant="outline" render={<a href="/auth/login">Sign in</a>}>
                Sign in
              </Button>
            ) : null}
            {onRetry ? <Button variant="outline" onClick={onRetry}>Try again</Button> : null}
          </Empty>
        ) : items.length > 0 ? (
          groupQueueItemsByStatus(items).map((group) => (
            <section key={group.value} aria-labelledby={`queue-status-${group.value}`}>
              <div className="flex h-8 items-center gap-2 border-b border-border/50 bg-muted/15 px-4 text-[11px]">
                <Circle className="size-3 text-muted-foreground" />
                <h2 id={`queue-status-${group.value}`} className="font-medium text-foreground/80">{group.label}</h2>
                <span className="text-muted-foreground">{group.items.length}</span>
              </div>
              {group.items.map((item) => (
                <QueueRow key={`${item.projectId}-${item.issueId}`} item={item} organizationSlug={organizationSlug} selected={item.issueId === selectedIssueId} selectedForBulk={selectedIssueIds.has(item.issueId)} feedback={feedbackByIssueId[item.issueId]} showDetails={showDetails} onOpenIssue={onOpenIssue} onToggleSelection={onToggleSelection} onStatusChange={onStatusChange} onImportanceChange={onImportanceChange} />
              ))}
            </section>
          ))
        ) : (
          <Empty className="min-h-56 rounded-none border-0 p-0">
            <EmptyHeader>
              <EmptyMedia variant="icon"><Search /></EmptyMedia>
              <EmptyTitle>No issues found</EmptyTitle>
            </EmptyHeader>
          </Empty>
        )}
      </div>
    </div>
  );
}
