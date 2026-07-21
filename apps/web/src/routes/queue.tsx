import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams, useSearch } from "@tanstack/react-router";
import { useLiveQuery } from "@tanstack/react-db";
import { Archive, CircleDot, Layers3 } from "lucide-react";

import { Kbd } from "@/components/ui/kbd";
import { Button } from "@/components/ui/button";
import { ApiError, createIssue, getCurrentUser } from "@/lib/api";
import { matchesQueueAdvancedFilter, toQueueItem } from "../data/queue";
import { useAllIssues } from "../hooks/use-all-issues";
import { useTeamIssues } from "../hooks/use-team-issues";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";
import { createQueueCommandGroups } from "../components/queue/queue-command-groups";
import { QueueList } from "../components/queue/queue-list";
import { QueueToolbar } from "../components/queue/queue-toolbar";
import { QueueBulkActionBar, QueueDisplayMenu, QueueFilterChips, QueueFilterMenu } from "../components/queue/queue-controls";
import { ProjectHeader, type ProjectViewTab } from "../components/project/project-header";
import { ProjectShell } from "../components/project/project-shell";
import { ProjectSidebar } from "../components/project/project-sidebar";
import type { QueueBulkAction, QueueFilter, QueueMutationFeedback, QueueView } from "../components/queue/types";
import { moveQueueSelection, parseQueueSearch, serializeQueueSearch, type QueueSearchState } from "../data/queue-search";
import { LazyIssueCreateDialog } from "../components/issues/lazy-issue-create-dialog";
import type { IssueStatus } from "../components/issues/issue-status-menu";
import type { IssueImportance } from "../components/issues/issue-importance-menu";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";
import { createIssueMetadataCollection, updateIssueMetadata } from "../lib/metadata-sync";

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
  const { filter, view, query, showDetails, advancedFilter } = searchState;
  const [selectedIssueId, setSelectedIssueId] = useState<string | null>(null);
  const [selectedIssueIds, setSelectedIssueIds] = useState<Set<string>>(() => new Set());
  const [feedbackByIssueId, setFeedbackByIssueId] = useState<Record<string, QueueMutationFeedback>>({});
  const [createOpen, setCreateOpen] = useState(false);
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const queryClient = useQueryClient();

  const updateSearch = (next: Partial<QueueSearchState>, replace = false) => {
    void navigate({ replace, search: () => serializeQueueSearch({ ...searchState, ...next }) as never });
  };
  const shortcuts = useMemo(() => [
    { keys: ["c"], onTrigger: () => setCreateOpen(true) },
    { keys: ["g", "b"], onTrigger: () => updateSearch({ view: "backlog" }) },
    { keys: ["f", "r"], onTrigger: () => updateSearch({ filter: "ready" }) },
  ], [searchState]);

  const meQuery = useQuery({
    queryKey: ["auth", "me"],
    queryFn: getCurrentUser,
    retry: false,
  });
  const navigationQuery = useNavigation();
  const organizationSlug = organizationSlugProp ?? toOrganizationSlug(navigationQuery.data?.organizations[0]?.name ?? "Riichi");
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
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
  const applyBulkAction = async (action: QueueBulkAction) => {
    const selected = allItems.filter((item) => selectedIssueIds.has(item.issueId));
    await Promise.all(selected.map(async (item) => {
      try {
        if (action.kind === "status") await statusMutation.mutateAsync({ item, status: action.value });
        else await importanceMutation.mutateAsync({ item, importance: action.value });
      } catch {
        // The individual mutation owns the rejected acknowledgement for this row.
      }
    }));
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
      const matchesView =
        view === "all" ||
        (view === "active" ? item.state !== "held" : item.state === "held");
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
      if (!["j", "k", "ArrowDown", "ArrowUp", "Enter", "Escape"].includes(event.key)) return;
      event.preventDefault();
      if (event.key === "Escape") {
        setSelectedIssueId(null);
        return;
      }
      if (event.key === "Enter") {
        const selected = visibleItems.find((item) => item.issueId === selectedIssueId);
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
  }, [navigate, organizationSlug, selectProject, selectedIssueId, visibleItems]);

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
  ];

  const loading = meQuery.isPending || queueQuery.isPending;
  const error = meQuery.error ?? queueQuery.error;
  const displayError = error;
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
        actions={<><QueueFilterMenu items={allItems} advancedFilter={advancedFilter} onAdvancedFilterChange={(nextFilter) => updateSearch({ advancedFilter: nextFilter })} /><QueueDisplayMenu showDetails={showDetails} onShowDetailsChange={(nextShowDetails) => updateSearch({ showDetails: nextShowDetails })} /></>}
      />
      <QueueToolbar
        query={query}
        onQueryChange={(nextQuery) => updateSearch({ query: nextQuery }, true)}
        refreshing={queueQuery.isFetching}
        onRefresh={() => void queueQuery.refetch()}
        onCreate={() => setCreateOpen(true)}
      />
      {selectedIssueIds.size > 0 ? <QueueBulkActionBar count={selectedIssueIds.size} onSelectAll={() => setSelectedIssueIds(new Set(visibleItems.map((item) => item.issueId)))} onClear={() => setSelectedIssueIds(new Set())} onApply={(action) => void applyBulkAction(action)} /> : null}
      <QueueFilterChips state={searchState} items={allItems} onChange={(next) => updateSearch(next, true)} />
      <QueueList
        organizationSlug={organizationSlug}
        items={visibleItems}
        selectedIssueId={selectedIssueId}
        feedbackByIssueId={feedbackByIssueId}
        selectedIssueIds={selectedIssueIds}
        showDetails={showDetails}
        loading={loading}
        error={displayError instanceof Error ? displayError : undefined}
        onRetry={retry}
        authRequired={displayError instanceof ApiError && displayError.status === 401}
        onOpenIssue={(item) => {
          setSelectedIssueId(item.issueId);
          selectProject(item.projectId);
          void navigate({ to: "/$organizationSlug/teams/$teamKey/issues/$issueId", params: { organizationSlug, teamKey: item.teamKey, issueId: item.issueId } });
        }}
        onStatusChange={(item, status) => statusMutation.mutate({ item, status })}
        onImportanceChange={(item, importance) => importanceMutation.mutate({ item, importance })}
        onToggleSelection={(item, checked) => setSelectedIssueIds((current) => { const next = new Set(current); if (checked) next.add(item.issueId); else next.delete(item.issueId); return next; })}
      />
      <LazyIssueCreateDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        onSubmit={(input) => createMutation.mutate(input)}
        submitting={createMutation.isPending}
      />
    </ProjectShell>
  );
}
