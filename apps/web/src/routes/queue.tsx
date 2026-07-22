import { useCallback, useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams, useSearch } from "@tanstack/react-router";
import { Link } from "@tanstack/react-router";
import { useLiveQuery } from "@tanstack/react-db";
import { Archive, CircleDot, ExternalLink, Layers3, UserRound, X } from "lucide-react";

import { Kbd } from "@/components/ui/kbd";
import { Button } from "@/components/ui/button";
import { ApiError, createIssue, createOnboardingSample, deleteProjectSavedView, deleteSavedView, getCurrentUser, getProjectSavedViews, getSavedViews, saveProjectSavedView, saveSavedView } from "@/lib/api";
import { addQueueLabel, matchesQueueAdvancedFilter, matchesQueueView, toQueueItem } from "../data/queue";
import { useAllIssues } from "../hooks/use-all-issues";
import { useTeamIssues } from "../hooks/use-team-issues";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";
import { createQueueCommandGroups } from "../components/queue/queue-command-groups";
import { QueueList } from "../components/queue/queue-list";
import { QueueToolbar } from "../components/queue/queue-toolbar";
import { QueueBulkActionBar, QueueBulkResultSummary, QueueDisplayMenu, QueueFilterChips, QueueFilterMenu, QueueSavedViews } from "../components/queue/queue-controls";
import { ProjectHeader, type ProjectViewTab } from "../components/project/project-header";
import { ProjectShell } from "../components/project/project-shell";
import { ProjectSidebar } from "../components/project/project-sidebar";
import type { QueueBulkAction, QueueBulkResult, QueueFilter, QueueMutationFeedback, QueueView } from "../components/queue/types";
import { moveQueueSelection, parseQueueSearch, serializeQueueSearch, type QueueSearchState } from "../data/queue-search";
import { LazyIssueCreateDialog } from "../components/issues/lazy-issue-create-dialog";
import type { IssueStatus } from "../components/issues/issue-status-menu";
import type { IssueImportance } from "../components/issues/issue-importance-menu";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";
import { createIssueMetadataCollection, updateIssueMetadata } from "../lib/metadata-sync";

function IssuePeek({ item, onClose, onExpand }: { item: ReturnType<typeof toQueueItem>; onClose: () => void; onExpand: () => void }) {
  return (
    <aside className="flex w-full shrink-0 flex-col border-t border-border/60 bg-background md:w-[360px] md:border-l md:border-t-0" aria-label="Issue preview">
      <div className="flex items-start justify-between gap-3 border-b border-border/60 px-4 py-3">
        <div className="min-w-0">
          <p className="font-mono text-[10px] text-muted-foreground">{item.id}</p>
          <h2 className="mt-1 text-sm font-medium leading-snug">{item.title}</h2>
        </div>
        <Button variant="ghost" size="icon" className="size-7 shrink-0" onClick={onClose} aria-label="Close issue preview"><X className="size-4" /></Button>
      </div>
      <div className="grid gap-3 overflow-auto p-4 text-xs">
        <div className="grid grid-cols-2 gap-2">
          <div><p className="text-muted-foreground">Status</p><p className="mt-1 font-medium">{item.status.replaceAll("_", " ")}</p></div>
          <div><p className="text-muted-foreground">Priority</p><p className="mt-1 font-medium">{item.importance}</p></div>
          <div><p className="text-muted-foreground">Project</p><p className="mt-1 font-medium">{item.projectName}</p></div>
          <div><p className="text-muted-foreground">Age</p><p className="mt-1 font-medium">{item.age.replace(" in queue", "")}</p></div>
        </div>
        <div><p className="text-muted-foreground">Next action</p><p className="mt-1 rounded-md border border-border/60 bg-muted/20 p-2 text-foreground/80">{item.reason}</p></div>
        {item.labels.length > 0 ? <div><p className="text-muted-foreground">Labels</p><p className="mt-1 text-foreground/80">{item.labels.join(" · ")}</p></div> : null}
        <div className="flex flex-wrap gap-2 pt-2">
          <Button size="sm" className="h-8" onClick={onExpand}><ExternalLink className="mr-1.5 size-3.5" />Expand issue <span className="ml-1 font-mono text-[10px]">E</span></Button>
          <Button variant="outline" size="sm" className="h-8" onClick={onClose}>Back to queue <span className="ml-1 font-mono text-[10px]">Esc</span></Button>
        </div>
      </div>
    </aside>
  );
}

export function WorkspaceQueuePage() {
  const { organizationSlug } = useParams({ from: "/$organizationSlug/issues" });
  return <QueuePage organizationSlug={organizationSlug} />;
}

export function LegacyWorkspaceRedirect() {
  const navigate = useNavigate();
  const navigationQuery = useNavigation();
  useEffect(() => {
    const name = navigationQuery.data?.organizations[0]?.name;
    if (name) void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug: toOrganizationSlug(name) }, replace: true });
  }, [navigate, navigationQuery.data]);
  return <div className="grid min-h-svh place-items-center text-sm text-muted-foreground">Loading workspace…</div>;
}

export function QueuePage({ initialFilter = "all", initialView = "all", teamId, organizationSlug: organizationSlugProp }: { initialFilter?: QueueFilter; initialView?: QueueView; teamId?: string; organizationSlug?: string }) {
  const rawSearch = useSearch({ strict: false }) as Record<string, unknown>;
  const parsedSearch = useMemo(() => parseQueueSearch(rawSearch), [rawSearch]);
  const searchState: QueueSearchState = useMemo(() => ({
    ...parsedSearch,
    filter: parsedSearch.filter === "all" ? initialFilter : parsedSearch.filter,
    view: parsedSearch.view === "all" ? initialView : parsedSearch.view,
  }), [initialFilter, initialView, parsedSearch]);
  const { filter, view, query, showDetails, advancedFilter, peekIssueId } = searchState;
  const guided = rawSearch.guide === "1";
  const [selectedIssueId, setSelectedIssueId] = useState<string | null>(null);
  const [selectedIssueIds, setSelectedIssueIds] = useState<Set<string>>(() => new Set());
  const [feedbackByIssueId, setFeedbackByIssueId] = useState<Record<string, QueueMutationFeedback>>({});
  const [bulkResult, setBulkResult] = useState<QueueBulkResult | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const queryClient = useQueryClient();

  const updateSearch = useCallback((next: Partial<QueueSearchState>, replace = false) => {
    void navigate({ replace, search: () => serializeQueueSearch({ ...searchState, ...next }) as never });
  }, [navigate, searchState]);
  const shortcuts = useMemo(() => [
    { keys: ["c"], onTrigger: () => setCreateOpen(true) },
    { keys: ["g", "b"], onTrigger: () => updateSearch({ view: "backlog" }) },
    { keys: ["g", "m"], onTrigger: () => updateSearch({ view: "my_work" }) },
    { keys: ["f", "r"], onTrigger: () => updateSearch({ filter: "ready" }) },
  ], [searchState]);

  const meQuery = useQuery({
    queryKey: ["auth", "me"],
    queryFn: getCurrentUser,
    retry: false,
  });
  const navigationQuery = useNavigation();
  const savedViewsQuery = useQuery({ queryKey: ["saved-views"], queryFn: getSavedViews, retry: false });
  const organizationSlug = organizationSlugProp ?? toOrganizationSlug(navigationQuery.data?.organizations[0]?.name ?? "Riichi");
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const projectSavedViewsQuery = useQuery({ queryKey: ["saved-views", "project", projectId], queryFn: () => getProjectSavedViews(projectId!), enabled: Boolean(projectId), retry: false });
  const savedViews = useMemo(() => [...(projectSavedViewsQuery.data ?? []), ...(savedViewsQuery.data ?? [])], [projectSavedViewsQuery.data, savedViewsQuery.data]);
  const projectName = activeMembership?.project_name ?? "riichi";
  const allIssuesQuery = useAllIssues();
  const teamIssuesQuery = useTeamIssues(teamId ?? "");
  const queueQuery = teamId ? teamIssuesQuery : allIssuesQuery;
  const metadataCollection = useMemo(
    () => (projectId ? createIssueMetadataCollection(projectId) : null),
    [projectId],
  );
  const metadataQuery = useLiveQuery(() => metadataCollection, [metadataCollection]);
  const createMutation = useMutation({
    mutationFn: (input: { title: string; body: string; parent_issue_id?: string }) => {
      if (!projectId) throw new Error("No project membership is available.");
      return createIssue(projectId, input);
    },
    onSuccess: (issue) => {
      setCreateOpen(false);
      void queryClient.invalidateQueries({ queryKey: ["issues", "all"] });
      void navigate({ to: "/$organizationSlug/teams/$teamKey/issues/$issueId", params: { organizationSlug, teamKey: issue.team_key, issueId: issue.id } });
    },
  });
  const saveViewMutation = useMutation({
    mutationFn: ({ name, scope }: { name: string; scope: "project" | "personal" }) => {
      const filters = serializeQueueSearch({ ...searchState, peekIssueId: undefined });
      if (scope === "project") {
        if (!projectId) throw new Error("No active project is available.");
        return saveProjectSavedView(projectId, name, filters);
      }
      return saveSavedView(name, filters);
    },
    onSuccess: (_view, { scope }) => void queryClient.invalidateQueries({ queryKey: scope === "project" ? ["saved-views", "project", projectId] : ["saved-views"] }),
  });
  const deleteViewMutation = useMutation({
    mutationFn: (view: { id: string; visibility: "project" | "personal" }) => view.visibility === "project" && projectId ? deleteProjectSavedView(projectId, view.id) : deleteSavedView(view.id),
    onSuccess: (_result, view) => void queryClient.invalidateQueries({ queryKey: view.visibility === "project" ? ["saved-views", "project", projectId] : ["saved-views"] }),
  });
  const sampleMutation = useMutation({
    mutationFn: () => {
      if (!projectId) throw new Error("No project membership is available.");
      return createOnboardingSample(projectId);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["issues", "all"] });
      void queryClient.invalidateQueries({ queryKey: ["agents"] });
      void queryClient.invalidateQueries({ queryKey: ["approvals"] });
      void queryClient.invalidateQueries({ queryKey: ["inbox"] });
    },
  });
  const statusMutation = useMutation({
    mutationFn: async ({ item, status }: { item: ReturnType<typeof toQueueItem>; status: IssueStatus }) => {
      return updateIssueMetadata(metadataCollection, item.projectId, item.issueId, { status });
    },
    onMutate: ({ item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "pending" } }));
    },
    onSuccess: (_result, { item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "confirmed" } }));
      void queryClient.invalidateQueries({ queryKey: ["issues", "all"] });
      if (teamId) void queryClient.invalidateQueries({ queryKey: ["issues", "team", teamId] });
    },
    onError: (error, { item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "rejected", message: error instanceof Error ? error.message : "Update failed" } }));
    },
  });
  const importanceMutation = useMutation({
    mutationFn: async ({ item, importance }: { item: ReturnType<typeof toQueueItem>; importance: IssueImportance }) => {
      return updateIssueMetadata(metadataCollection, item.projectId, item.issueId, { importance });
    },
    onMutate: ({ item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "pending" } }));
    },
    onSuccess: (_result, { item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "confirmed" } }));
      void queryClient.invalidateQueries({ queryKey: ["issues", "all"] });
      if (teamId) void queryClient.invalidateQueries({ queryKey: ["issues", "team", teamId] });
    },
    onError: (error, { item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "rejected", message: error instanceof Error ? error.message : "Update failed" } }));
    },
  });
  const labelsMutation = useMutation({
    mutationFn: async ({ item, label }: { item: ReturnType<typeof toQueueItem>; label: string }) => {
      return updateIssueMetadata(metadataCollection, item.projectId, item.issueId, { labels: addQueueLabel(item.labels, label) });
    },
    onMutate: ({ item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "pending" } }));
    },
    onSuccess: (_result, { item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "confirmed" } }));
      void queryClient.invalidateQueries({ queryKey: ["issues", "all"] });
      if (teamId) void queryClient.invalidateQueries({ queryKey: ["issues", "team", teamId] });
    },
    onError: (error, { item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "rejected", message: error instanceof Error ? error.message : "Update failed" } }));
    },
  });
  const assigneeMutation = useMutation({
    mutationFn: async ({ item, accountId }: { item: ReturnType<typeof toQueueItem>; accountId: string }) => {
      return updateIssueMetadata(metadataCollection, item.projectId, item.issueId, { assignee_account_id: accountId });
    },
    onMutate: ({ item }) => setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "pending" } })),
    onSuccess: (_result, { item }) => {
      setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "confirmed" } }));
      void queryClient.invalidateQueries({ queryKey: ["issues", "all"] });
      if (teamId) void queryClient.invalidateQueries({ queryKey: ["issues", "team", teamId] });
    },
    onError: (error, { item }) => setFeedbackByIssueId((current) => ({ ...current, [item.issueId]: { state: "rejected", message: error instanceof Error ? error.message : "Update failed" } })),
  });
  const applyBulkAction = async (action: QueueBulkAction) => {
    const selected = allItems.filter((item) => selectedIssueIds.has(item.issueId));
    setBulkResult(null);
    const outcomes = await Promise.all(selected.map(async (item) => {
      try {
        if (action.kind === "status") await statusMutation.mutateAsync({ item, status: action.value });
        else if (action.kind === "importance") await importanceMutation.mutateAsync({ item, importance: action.value });
        else if (action.kind === "label") await labelsMutation.mutateAsync({ item, label: action.value });
        else await assigneeMutation.mutateAsync({ item, accountId: action.value });
        return true;
      } catch {
        // The individual mutation owns the rejected acknowledgement for this row.
        return false;
      }
    }));
    const confirmed = outcomes.filter(Boolean).length;
    setBulkResult({ total: outcomes.length, confirmed, rejected: outcomes.length - confirmed });
    setSelectedIssueIds(new Set());
  };
  const allItems = useMemo(
    () => {
      const metadataById = new Map(
        (metadataQuery.data ?? []).map((issue) => [issue.id, issue]),
      );
      return queueQuery.data?.map((issue) => {
        const metadata = metadataById.get(issue.id);
        return toQueueItem(
          metadata
            ? {
                ...issue,
                title: metadata.title,
                status: metadata.status,
                importance: metadata.importance,
                agent_eligible: metadata.agent_eligible,
                spec_complete: metadata.spec_complete,
                rank: metadata.rank,
                labels: metadata.labels,
                dispatch_version: issue.dispatch_version,
              }
            : issue,
        );
      }) ?? [];
    },
    [metadataQuery.data, queueQuery.data],
  );
  const commandGroups = useMemo(
    () => createQueueCommandGroups({
      onCreate: () => setCreateOpen(true),
      onFilterChange: (nextFilter) => updateSearch({ filter: nextFilter }),
      onViewChange: (nextView) => updateSearch({ view: nextView }),
      onStatusFilterChange: (status) => updateSearch({ advancedFilter: { ...searchState.advancedFilter, status } }),
      onImportanceFilterChange: (importance) => updateSearch({ advancedFilter: { ...searchState.advancedFilter, importance } }),
      onQueryChange: (nextQuery) => updateSearch({ query: nextQuery }, true),
      items: allItems,
    }),
    [allItems, searchState],
  );

  useEffect(() => {
    // Queue rows are fully replicated by human_issue_sync when Electric is
    // enabled. Keep SSE only for the authenticated API fallback during the
    // migration window.
    if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED === "true" || !projectId || typeof EventSource === "undefined") return;
    const events = new EventSource(
      `/api/v1/projects/${encodeURIComponent(projectId)}/events`,
      { withCredentials: true },
    );
    const refresh = () => {
      void queryClient.invalidateQueries({ queryKey: ["issues", "all"] });
    };
    events.onmessage = refresh;
    events.addEventListener("issue_changed", refresh);
    events.addEventListener("lease_changed", refresh);
    return () => events.close();
  }, [queryClient, projectId]);

  const visibleItems = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return allItems.filter((item) => {
      const matchesFilter = filter === "all" || item.state === filter;
      const matchesView = matchesQueueView(item, view, meQuery.data?.account_id);
      const matchesQuery =
        !normalizedQuery ||
        `${item.id} ${item.title} ${item.description} ${item.reason}`
          .toLowerCase()
          .includes(normalizedQuery);
      return matchesFilter && matchesQueueAdvancedFilter(item, advancedFilter, meQuery.data?.account_id) && matchesView && matchesQuery;
    });
  }, [allItems, advancedFilter, filter, meQuery.data?.account_id, query, view]);

  useEffect(() => {
    if (selectedIssueId && !visibleItems.some((item) => item.issueId === selectedIssueId)) {
      setSelectedIssueId(visibleItems[0]?.issueId ?? null);
    }
  }, [selectedIssueId, visibleItems]);

  useEffect(() => {
    setSelectedIssueIds((current) => {
      const visibleIds = new Set(visibleItems.map((item) => item.issueId));
      const next = new Set([...current].filter((issueId) => visibleIds.has(issueId)));
      return next.size === current.size ? current : next;
    });
  }, [visibleItems]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']") || event.metaKey || event.ctrlKey || event.altKey) return;
      if (!["j", "k", "ArrowDown", "ArrowUp", "Enter", "Escape", "e", "E"].includes(event.key)) return;
      event.preventDefault();
      if (event.key === "Escape") {
        setSelectedIssueId(null);
        updateSearch({ peekIssueId: undefined }, true);
        return;
      }
      if (event.key === "Enter") {
        const selected = visibleItems.find((item) => item.issueId === selectedIssueId);
        if (selected) updateSearch({ peekIssueId: selected.issueId }, true);
        return;
      }
      if (event.key.toLowerCase() === "e") {
        const selected = visibleItems.find((item) => item.issueId === (peekIssueId ?? selectedIssueId));
        if (selected) {
          selectProject(selected.projectId);
          void navigate({ to: "/$organizationSlug/teams/$teamKey/issues/$issueId", params: { organizationSlug, teamKey: selected.teamKey, issueId: selected.issueId } });
        }
        return;
      }
      if (visibleItems.length === 0) return;
      const direction = event.key === "j" || event.key === "ArrowDown" ? 1 : -1;
      setSelectedIssueId(moveQueueSelection(visibleItems.map((item) => item.issueId), selectedIssueId, direction));
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate, organizationSlug, peekIssueId, selectProject, selectedIssueId, updateSearch, visibleItems]);

  const queueViews: ProjectViewTab[] = [
    { value: "all", label: "All issues", icon: Layers3 },
    {
      value: "active",
      label: "Active",
      icon: CircleDot,
      count: allItems.filter((item) => item.state !== "held").length,
    },
    {
      value: "backlog",
      label: "Backlog",
      icon: Archive,
      count: allItems.filter((item) => item.state === "held").length,
    },
    {
      value: "my_work",
      label: "My work",
      icon: UserRound,
      count: allItems.filter((item) => item.assigneeAccountId === meQuery.data?.account_id).length,
    },
  ];

  const loading = meQuery.isPending || queueQuery.isPending;
  const error = meQuery.error ?? queueQuery.error;
  const displayError = meQuery.error ?? (queueQuery.data ? undefined : queueQuery.error);
  const retry = () => {
    void meQuery.refetch();
    void queueQuery.refetch();
  };

  return (
    <ProjectShell
      commandGroups={commandGroups}
      shortcuts={shortcuts}
      sidebar={
        <ProjectSidebar
          onCreate={() => setCreateOpen(true)}
          projectName={projectName}
          navigation={navigationQuery.data}
          memberships={meQuery.data?.memberships}
          activeProjectId={projectId}
          onProjectChange={selectProject}
          onLogout={appLogout}
          onNavigate={(label) => {
            if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
            if (label === "Link GitHub") void navigate({ to: "/$organizationSlug/integrations", params: { organizationSlug } });
            if (label === "Invite people") void navigate({ to: "/$organizationSlug/settings", params: { organizationSlug } });
          }}
          userName={meQuery.data?.display_name ?? "Alex Morgan"}
          avatarUrl={meQuery.data?.avatar_url}
        />
      }
      footer={
        <footer className="flex h-8 shrink-0 items-center gap-3 border-t border-border/60 px-4 text-[10px] text-muted-foreground">
          <span className="flex items-center gap-1.5 text-foreground/70">
            <span className="size-1.5 rounded-full bg-emerald-400" /> {displayError ? "Connection needs attention" : "All systems operational"}
          </span>
          <span className="ml-auto">{projectId ? "synced from server" : "waiting for project"}</span>
          <Kbd className="h-5 bg-muted px-1.5 text-[10px]">⌘ K</Kbd>
          <span>Command menu</span>
        </footer>
      }
    >
      <ProjectHeader
        view={view}
        views={queueViews}
        onViewChange={(nextView) => updateSearch({ view: nextView as QueueView })}
        actions={<><QueueSavedViews views={savedViews} onApply={(savedView) => updateSearch(parseQueueSearch(savedView.filters), true)} onSave={(name, scope) => saveViewMutation.mutate({ name, scope })} onDelete={(savedView) => deleteViewMutation.mutate(savedView)} /><QueueFilterMenu items={allItems} advancedFilter={advancedFilter} onAdvancedFilterChange={(nextFilter) => updateSearch({ advancedFilter: nextFilter })} /><QueueDisplayMenu showDetails={showDetails} onShowDetailsChange={(nextShowDetails) => updateSearch({ showDetails: nextShowDetails })} /></>}
      />
      <QueueToolbar
        query={query}
        onQueryChange={(nextQuery) => updateSearch({ query: nextQuery }, true)}
        refreshing={queueQuery.isFetching}
        onRefresh={() => void queueQuery.refetch()}
        onCreate={() => setCreateOpen(true)}
      />
      {guided ? <section className="mx-4 my-3 grid gap-3 rounded-lg border border-primary/30 bg-primary/5 p-4 text-sm" aria-labelledby="getting-started-title"><div className="flex items-start justify-between gap-3"><div><h2 id="getting-started-title" className="font-medium">A quick tour of Riichi</h2><p className="mt-1 text-xs text-muted-foreground">Start with the queue, then explore the human and agent workflows.</p></div><Button variant="ghost" size="sm" className="h-8" onClick={() => void navigate({ search: () => ({ ...serializeQueueSearch(searchState) }) as never, replace: true })}>Dismiss</Button></div>{activeMembership?.role === "owner" || activeMembership?.role === "admin" ? <div className="flex flex-wrap items-center gap-2 rounded-md border border-border/60 bg-background/50 p-3 text-xs"><span className="text-muted-foreground">Want real examples?</span><Button size="sm" className="h-8" onClick={() => sampleMutation.mutate()} disabled={sampleMutation.isPending}>{sampleMutation.isPending ? "Creating sample…" : "Create guided sample"}</Button>{sampleMutation.isSuccess ? <span role="status" className="text-emerald-400">Sample workflow is ready.</span> : null}{sampleMutation.error ? <span role="alert" className="text-destructive">{sampleMutation.error.message}</span> : null}</div> : null}<div className="grid gap-2 sm:grid-cols-4"><Link className="rounded-md border border-border/60 bg-background/60 p-2 text-xs hover:bg-muted/50" to="/$organizationSlug/issues" params={{ organizationSlug }}><span className="font-medium">1. Triage work</span><span className="mt-1 block text-muted-foreground">Filter and open an issue.</span></Link><Link className="rounded-md border border-border/60 bg-background/60 p-2 text-xs hover:bg-muted/50" to="/$organizationSlug/agents" params={{ organizationSlug }}><span className="font-medium">2. Agent workflow</span><span className="mt-1 block text-muted-foreground">Inspect claim and report surfaces.</span></Link><Link className="rounded-md border border-border/60 bg-background/60 p-2 text-xs hover:bg-muted/50" to="/$organizationSlug/approvals" params={{ organizationSlug }}><span className="font-medium">3. Approvals</span><span className="mt-1 block text-muted-foreground">Review versioned decisions.</span></Link><Link className="rounded-md border border-border/60 bg-background/60 p-2 text-xs hover:bg-muted/50" to="/$organizationSlug/inbox" params={{ organizationSlug }}><span className="font-medium">4. Inbox</span><span className="mt-1 block text-muted-foreground">Follow actionable notifications.</span></Link></div></section> : null}
      {selectedIssueIds.size > 0 ? <QueueBulkActionBar count={selectedIssueIds.size} labels={[...new Set(allItems.flatMap((item) => item.labels))].sort()} accountId={meQuery.data?.account_id} onSelectAll={() => setSelectedIssueIds(new Set(visibleItems.map((item) => item.issueId)))} onClear={() => setSelectedIssueIds(new Set())} onApply={(action) => void applyBulkAction(action)} /> : null}
      {bulkResult ? <QueueBulkResultSummary result={bulkResult} onDismiss={() => setBulkResult(null)} /> : null}
      <QueueFilterChips state={searchState} items={allItems} onChange={(next) => updateSearch(next, true)} />
      <div className="flex min-h-0 flex-1 flex-col md:flex-row">
      <QueueList
        organizationSlug={organizationSlug}
        items={visibleItems}
        selectedIssueId={selectedIssueId}
        feedbackByIssueId={feedbackByIssueId}
        selectedIssueIds={selectedIssueIds}
        showDetails={showDetails}
        loading={loading}
        stale={Boolean(queueQuery.error && queueQuery.data)}
        error={displayError instanceof Error ? displayError : undefined}
        onRetry={retry}
        authRequired={displayError instanceof ApiError && displayError.status === 401}
        onOpenIssue={(item) => {
          setSelectedIssueId(item.issueId);
          updateSearch({ peekIssueId: item.issueId }, true);
        }}
        onStatusChange={(item, status) => statusMutation.mutate({ item, status })}
        onImportanceChange={(item, importance) => importanceMutation.mutate({ item, importance })}
        onToggleSelection={(item, checked) => setSelectedIssueIds((current) => { const next = new Set(current); if (checked) next.add(item.issueId); else next.delete(item.issueId); return next; })}
      />
      {peekIssueId ? (() => {
        const peekItem = allItems.find((item) => item.issueId === peekIssueId);
        return peekItem ? <IssuePeek item={peekItem} onClose={() => updateSearch({ peekIssueId: undefined }, true)} onExpand={() => { selectProject(peekItem.projectId); void navigate({ to: "/$organizationSlug/teams/$teamKey/issues/$issueId", params: { organizationSlug, teamKey: peekItem.teamKey, issueId: peekItem.issueId } }); }} /> : null;
      })() : null}
      </div>
      <LazyIssueCreateDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        onSubmit={(input) => createMutation.mutate(input)}
        submitting={createMutation.isPending}
      />
    </ProjectShell>
  );
}
