import { Filter, SlidersHorizontal, X } from "lucide-react";
import * as React from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { DropdownMenu, DropdownMenuCheckboxItem, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuSeparator, DropdownMenuSub, DropdownMenuSubContent, DropdownMenuSubTrigger, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { activeQueueFilterChips, type QueueSearchState } from "@/data/queue-search";
import type { QueueItem } from "@/data/queue";
import { issueImportanceLabel, type IssueImportance } from "@/components/issues/issue-importance-menu";
import { issueStatuses, type IssueStatus } from "@/components/issues/issue-status-menu";
import type { QueueAdvancedFilter, QueueBulkAction, QueueBulkResult } from "./types";
import type { SavedView } from "@/lib/api";

export function requiresBulkActionConfirmation(action: QueueBulkAction) {
  return action.kind === "status" && action.value === "canceled";
}

export function bulkResultCopy(result: QueueBulkResult) {
  const parts = [`${result.confirmed} saved`];
  if (result.rejected > 0) parts.push(`${result.rejected} rejected`);
  return parts.join(", ");
}

export function QueueBulkResultSummary({ result, onDismiss }: { result: QueueBulkResult; onDismiss: () => void }) {
  return <div role="status" className="flex items-center gap-2 border-b border-border/50 bg-muted/10 px-4 py-1.5 text-xs"><span>{bulkResultCopy(result)}{result.total > 1 ? ` of ${result.total}` : ""}</span>{result.rejected > 0 ? <span className="text-destructive">Review rejected rows.</span> : null}<Button variant="ghost" size="sm" className="ml-auto h-6 px-2 text-xs" onClick={onDismiss}>Dismiss</Button></div>;
}

export function QueueSavedViews({ views, onApply, onSave, onDelete }: {
  views: SavedView[];
  onApply: (view: SavedView) => void;
  onSave: (name: string, scope: "project" | "personal") => void;
  onDelete: (view: SavedView) => void;
}) {
  const [open, setOpen] = React.useState(false);
  const [name, setName] = React.useState("");
  const [scope, setScope] = React.useState<"project" | "personal">("project");
  const beginSave = (nextScope: "project" | "personal") => {
    setScope(nextScope);
    setOpen(true);
  };
  return <>
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="ghost" size="sm" className="h-8 gap-1.5 text-xs text-muted-foreground" aria-label="Saved views" />}>Views</DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        <DropdownMenuLabel>Saved views</DropdownMenuLabel>
        <DropdownMenuSeparator />
        {views.length ? views.map((view) => <div key={view.id} className="flex items-center gap-1 px-1">
          <DropdownMenuItem className="min-w-0 flex-1" onClick={() => onApply(view)}><span className="truncate">{view.name}</span><span className="ml-auto text-[10px] text-muted-foreground">{view.visibility}</span></DropdownMenuItem>
          <button type="button" className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground" aria-label={`Delete ${view.name}`} onClick={() => onDelete(view)}>×</button>
        </div>) : <p className="px-2 py-1.5 text-xs text-muted-foreground">No saved views yet.</p>}
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={() => beginSave("project")}>Save shared project view…</DropdownMenuItem>
        <DropdownMenuItem onClick={() => beginSave("personal")}>Save personal view…</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent>
        <DialogHeader><DialogTitle>Save current filters</DialogTitle><DialogDescription>{scope === "project" ? "This view will be shared with project members." : "This view will be available only on your account."}</DialogDescription></DialogHeader>
        <Input value={name} maxLength={80} autoFocus placeholder="e.g. urgent triage" onChange={(event) => setName(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && name.trim()) { onSave(name.trim(), scope); setName(""); setOpen(false); } }} />
        <DialogFooter><DialogClose render={<Button variant="outline" />}>Cancel</DialogClose><Button disabled={!name.trim()} onClick={() => { onSave(name.trim(), scope); setName(""); setOpen(false); }}>Save view</Button></DialogFooter>
      </DialogContent>
    </Dialog>
  </>;
}

export function QueueFilterMenu({ items, advancedFilter, onAdvancedFilterChange }: { items: QueueItem[]; advancedFilter: QueueAdvancedFilter; onAdvancedFilterChange: (filter: QueueAdvancedFilter) => void }) {
  const teams = [...new Map(items.map((item) => [item.teamKey, item.teamName])).entries()];
  const projects = [...new Map(items.map((item) => [item.projectId, item.projectName])).entries()];
  const labels = [...new Set(items.flatMap((item) => item.labels))].sort();
  const count = [advancedFilter.status, advancedFilter.importance, advancedFilter.teamKey, advancedFilter.projectId, advancedFilter.assignee, advancedFilter.label].filter((value) => value !== "all").length;
  const clear = () => onAdvancedFilterChange({ status: "all", importance: "all", teamKey: "all", projectId: "all", assignee: "all", label: "all" });
  return <DropdownMenu><DropdownMenuTrigger render={<Button variant="ghost" size="icon-sm" className="text-muted-foreground" aria-label={count ? `Filter, ${count} active` : "Filter"} />}><Filter /></DropdownMenuTrigger><DropdownMenuContent align="end" className="w-52"><DropdownMenuLabel>{count ? `${count} active filter${count === 1 ? "" : "s"}` : "Filter issues"}</DropdownMenuLabel><DropdownMenuSeparator /><DropdownMenuSub><DropdownMenuSubTrigger>Status</DropdownMenuSubTrigger><DropdownMenuSubContent className="w-44"><DropdownMenuRadioGroup value={advancedFilter.status} onValueChange={(value) => onAdvancedFilterChange({ ...advancedFilter, status: value as QueueAdvancedFilter["status"] })}><DropdownMenuRadioItem value="all">Any status</DropdownMenuRadioItem>{issueStatuses.map((status) => <DropdownMenuRadioItem key={status.value} value={status.value}>{status.label}</DropdownMenuRadioItem>)}</DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub><DropdownMenuSub><DropdownMenuSubTrigger>Importance</DropdownMenuSubTrigger><DropdownMenuSubContent className="w-44"><DropdownMenuRadioGroup value={advancedFilter.importance} onValueChange={(value) => onAdvancedFilterChange({ ...advancedFilter, importance: value as QueueAdvancedFilter["importance"] })}><DropdownMenuRadioItem value="all">Any importance</DropdownMenuRadioItem>{(["none", "low", "medium", "high", "urgent"] as IssueImportance[]).map((importance) => <DropdownMenuRadioItem key={importance} value={importance}>{issueImportanceLabel(importance)}</DropdownMenuRadioItem>)}</DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub><DropdownMenuSub><DropdownMenuSubTrigger>Assignee</DropdownMenuSubTrigger><DropdownMenuSubContent className="w-44"><DropdownMenuRadioGroup value={advancedFilter.assignee} onValueChange={(value) => onAdvancedFilterChange({ ...advancedFilter, assignee: value as QueueAdvancedFilter["assignee"] })}><DropdownMenuRadioItem value="all">Any assignee</DropdownMenuRadioItem><DropdownMenuRadioItem value="me">Assigned to me</DropdownMenuRadioItem><DropdownMenuRadioItem value="assigned">Anyone assigned</DropdownMenuRadioItem><DropdownMenuRadioItem value="unassigned">Unassigned</DropdownMenuRadioItem></DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub>{labels.length ? <DropdownMenuSub><DropdownMenuSubTrigger>Label</DropdownMenuSubTrigger><DropdownMenuSubContent className="max-h-72 w-52"><DropdownMenuRadioGroup value={advancedFilter.label} onValueChange={(value) => onAdvancedFilterChange({ ...advancedFilter, label: value })}><DropdownMenuRadioItem value="all">Any label</DropdownMenuRadioItem>{labels.map((label) => <DropdownMenuRadioItem key={label} value={label}>{label}</DropdownMenuRadioItem>)}</DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub> : null}{teams.length > 1 ? <DropdownMenuSub><DropdownMenuSubTrigger>Team</DropdownMenuSubTrigger><DropdownMenuSubContent className="w-48"><DropdownMenuRadioGroup value={advancedFilter.teamKey} onValueChange={(value) => onAdvancedFilterChange({ ...advancedFilter, teamKey: value })}><DropdownMenuRadioItem value="all">Any team</DropdownMenuRadioItem>{teams.map(([key, name]) => <DropdownMenuRadioItem key={key} value={key}>{name} · {key}</DropdownMenuRadioItem>)}</DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub> : null}{projects.length > 1 ? <DropdownMenuSub><DropdownMenuSubTrigger>Project</DropdownMenuSubTrigger><DropdownMenuSubContent className="max-h-72 w-52"><DropdownMenuRadioGroup value={advancedFilter.projectId} onValueChange={(value) => onAdvancedFilterChange({ ...advancedFilter, projectId: value })}><DropdownMenuRadioItem value="all">Any project</DropdownMenuRadioItem>{projects.map(([id, name]) => <DropdownMenuRadioItem key={id} value={id}>{name}</DropdownMenuRadioItem>)}</DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub> : null}<DropdownMenuSeparator /><DropdownMenuItem onClick={clear}>Clear advanced filters</DropdownMenuItem></DropdownMenuContent></DropdownMenu>;
}

export function QueueDisplayMenu({ showDetails, onShowDetailsChange }: { showDetails: boolean; onShowDetailsChange: (show: boolean) => void }) {
  return <DropdownMenu><DropdownMenuTrigger render={<Button variant="ghost" size="icon-sm" className="text-muted-foreground" aria-label="Display options" />}><SlidersHorizontal /></DropdownMenuTrigger><DropdownMenuContent align="end" className="w-48"><DropdownMenuLabel>Display options</DropdownMenuLabel><DropdownMenuSeparator /><DropdownMenuCheckboxItem checked={showDetails} onCheckedChange={(checked) => onShowDetailsChange(checked === true)}>Show status details</DropdownMenuCheckboxItem><DropdownMenuCheckboxItem checked={false} disabled>Show estimates</DropdownMenuCheckboxItem></DropdownMenuContent></DropdownMenu>;
}

export function QueueBulkActionBar({ count, labels, accountId, onSelectAll, onClear, onApply }: { count: number; labels: string[]; accountId?: string; onSelectAll: () => void; onClear: () => void; onApply: (action: QueueBulkAction) => void }) {
  const [confirmAction, setConfirmAction] = React.useState<QueueBulkAction | null>(null);
  const apply = (action: QueueBulkAction) => {
    if (requiresBulkActionConfirmation(action)) setConfirmAction(action);
    else onApply(action);
  };
  return <><div className="flex min-h-10 flex-wrap items-center gap-2 border-b border-border/50 bg-muted/20 px-4 py-1.5" role="toolbar" aria-label="Bulk issue actions"><span className="text-xs text-muted-foreground">{count} selected</span><Button variant="ghost" size="sm" onClick={onSelectAll}>Select visible</Button><DropdownMenu><DropdownMenuTrigger render={<Button variant="outline" size="sm">Change…</Button>} /><DropdownMenuContent align="start" className="w-48"><DropdownMenuSub><DropdownMenuSubTrigger>Status</DropdownMenuSubTrigger><DropdownMenuSubContent className="w-44"><DropdownMenuRadioGroup onValueChange={(value) => apply({ kind: "status", value: value as IssueStatus })}>{issueStatuses.map((status) => <DropdownMenuRadioItem key={status.value} value={status.value}>{status.label}</DropdownMenuRadioItem>)}</DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub><DropdownMenuSub><DropdownMenuSubTrigger>Priority</DropdownMenuSubTrigger><DropdownMenuSubContent className="w-44"><DropdownMenuRadioGroup onValueChange={(value) => apply({ kind: "importance", value: value as IssueImportance })}>{(["none", "low", "medium", "high", "urgent"] as IssueImportance[]).map((importance) => <DropdownMenuRadioItem key={importance} value={importance}>{issueImportanceLabel(importance)}</DropdownMenuRadioItem>)}</DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub>{accountId ? <DropdownMenuSub><DropdownMenuSubTrigger>Assignee</DropdownMenuSubTrigger><DropdownMenuSubContent className="w-44"><DropdownMenuItem onClick={() => apply({ kind: "assignee", value: accountId })}>Assign to me</DropdownMenuItem></DropdownMenuSubContent></DropdownMenuSub> : null}{labels.length ? <DropdownMenuSub><DropdownMenuSubTrigger>Add label</DropdownMenuSubTrigger><DropdownMenuSubContent className="max-h-72 w-44"><DropdownMenuRadioGroup onValueChange={(value) => apply({ kind: "label", value })}>{labels.map((label) => <DropdownMenuRadioItem key={label} value={label}>{label}</DropdownMenuRadioItem>)}</DropdownMenuRadioGroup></DropdownMenuSubContent></DropdownMenuSub> : null}</DropdownMenuContent></DropdownMenu><Button variant="ghost" size="sm" onClick={onClear}>Clear</Button></div><Dialog open={Boolean(confirmAction)} onOpenChange={(open) => { if (!open) setConfirmAction(null); }}><DialogContent><DialogHeader><DialogTitle>Cancel selected issues?</DialogTitle><DialogDescription>This changes {count} issue{count === 1 ? "" : "s"} to canceled. The server will confirm each mutation individually.</DialogDescription></DialogHeader><DialogFooter><DialogClose render={<Button variant="outline" />}>Keep issues</DialogClose><Button variant="destructive" onClick={() => { if (confirmAction) onApply(confirmAction); setConfirmAction(null); }}>Cancel issues</Button></DialogFooter></DialogContent></Dialog></>;
}

export function QueueFilterChips({ state, items, onChange }: { state: QueueSearchState; items: QueueItem[]; onChange: (next: Partial<QueueSearchState>) => void }) {
  const teamName = items.find((item) => item.teamKey === state.advancedFilter.teamKey)?.teamName;
  const projectName = items.find((item) => item.projectId === state.advancedFilter.projectId)?.projectName;
  const chips = activeQueueFilterChips(state, { team: teamName, project: projectName });
  if (chips.length === 0) return null;
  return (
    <div className="flex min-h-8 shrink-0 flex-wrap items-center gap-1 border-b border-border/40 px-4 py-1.5" aria-label="Active filters">
      {chips.map((chip) => (
        <button
          key={chip.id}
          type="button"
          className="inline-flex h-6 items-center gap-1 rounded-md border border-border bg-muted/45 px-2 text-[11px] text-foreground/80 transition-colors hover:bg-muted focus-visible:border-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          onClick={() => onChange(chip.clear)}
          aria-label={`Clear ${chip.label} filter`}
        >
          {chip.label}
          <X className="size-3 text-muted-foreground" aria-hidden="true" />
        </button>
      ))}
    </div>
  );
}
