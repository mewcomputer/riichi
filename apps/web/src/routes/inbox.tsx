import { useMemo, useState } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams, useSearch } from "@tanstack/react-router";
import { Bell, Check } from "lucide-react";

import { Empty, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Button } from "@/components/ui/button";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { getCurrentUser, getInbox, markInboxNotificationRead, type Notification } from "@/lib/api";
import { createNotificationCollection } from "@/lib/metadata-sync";
import { useActiveProject } from "@/hooks/use-active-project";
import { useAppLogout } from "@/hooks/use-app-logout";
import { useNavigation } from "@/hooks/use-navigation";
import { organizationSlug as toOrganizationSlug } from "@/lib/organization-slug";
import { filterNotifications, notificationAction, notificationStateLabel, notificationSummary, notificationTitle } from "@/data/inbox";

export function InboxPage() {
  const { organizationSlug } = useParams({ from: "/$organizationSlug/inbox" });
  const rawSearch = useSearch({ strict: false }) as Record<string, unknown>;
  const projectFilter = typeof rawSearch.project === "string" && rawSearch.project ? rawSearch.project : "all";
  const kindFilter = ["all", "comment", "approval", "assignment", "invitation", "takeover", "lease"].includes(String(rawSearch.kind)) ? String(rawSearch.kind) as Notification["kind"] | "all" : "all";
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const queryClient = useQueryClient();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const [readConfirmedIds, setReadConfirmedIds] = useState<Set<string>>(() => new Set());
  const collection = useMemo(() => createNotificationCollection(), []);
  const liveQuery = useLiveQuery(() => collection, [collection]);
  const inboxQuery = useQuery({ queryKey: ["inbox"], queryFn: () => getInbox({ limit: 100 }) });
  const replicatedNotifications = liveQuery.data as Notification[] | undefined;
  const serverNotifications = inboxQuery.data?.notifications ?? [];
  const serverById = new Map(serverNotifications.map((notification) => [notification.id, notification]));
  const notifications: Notification[] = replicatedNotifications
    ? replicatedNotifications.map((notification) => ({ ...notification, approval_state: serverById.get(notification.id)?.approval_state ?? notification.approval_state }))
    : serverNotifications.map((notification) => ({
    ...notification,
    recipient_account_id: notification.recipient_account_id ?? "",
    project_id: notification.project_id ?? null,
    issue_id: notification.issue_id ?? null,
    actor_id: notification.actor_id ?? null,
    read_at: notification.read_at ?? null,
  }));
  const visibleNotifications = filterNotifications(notifications, projectFilter, kindFilter);
  const unreadCount = visibleNotifications.filter((notification) => notification.read_at === null && !readConfirmedIds.has(notification.id)).length;
  const projects = useMemo(
    () => navigationQuery.data?.organizations.flatMap((organization) => organization.teams.flatMap((team) => team.projects.map((project) => ({ id: project.id, label: `${project.name} · ${team.key}` })))) ?? [],
    [navigationQuery.data],
  );
  const updateInboxFilters = (next: { project?: string; kind?: string }) => {
    const project = next.project ?? projectFilter;
    const kind = next.kind ?? kindFilter;
    void navigate({ replace: true, search: () => ({ ...(project === "all" ? {} : { project }), ...(kind === "all" ? {} : { kind }) }) as never });
  };
  const [readPendingIds, setReadPendingIds] = useState<Set<string>>(() => new Set());
  const markReadMutation = useMutation({
    mutationFn: async (notificationId: string) => {
      setReadPendingIds((current) => new Set(current).add(notificationId));
      await markInboxNotificationRead(notificationId);
      return notificationId;
    },
    onSuccess: (notificationId) => {
      setReadPendingIds((current) => { const next = new Set(current); next.delete(notificationId); return next; });
      setReadConfirmedIds((current) => new Set(current).add(notificationId));
      void queryClient.invalidateQueries({ queryKey: ["inbox"] });
    },
    onError: (_error, notificationId) => setReadPendingIds((current) => { const next = new Set(current); next.delete(notificationId); return next; }),
  });
  const organizationName = navigationQuery.data?.organizations.find(
    (organization) => toOrganizationSlug(organization.name) === organizationSlug,
  )?.name ?? "Riichi";

  return (
    <ProjectShell
      sidebar={<ProjectSidebar
        projectName={activeMembership?.project_name ?? "riichi"}
        navigation={navigationQuery.data}
        memberships={meQuery.data?.memberships}
        activeProjectId={projectId}
        onProjectChange={selectProject}
        onLogout={appLogout}
        avatarUrl={meQuery.data?.avatar_url}
        onSearch={() => undefined}
        onNavigate={(label) => {
          if (label === "Issues") void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } });
          if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
          if (label === "Approvals") void navigate({ to: "/$organizationSlug/approvals", params: { organizationSlug } });
        }}
        userName={meQuery.data?.display_name ?? "Alex Morgan"}
      />}
    >
      <main className="mx-auto w-full max-w-4xl px-4 py-5 sm:px-6 sm:py-8 lg:px-8">
        <header className="mb-5 flex flex-col items-stretch gap-4 border-b border-border/60 pb-5 sm:mb-6 sm:flex-row sm:items-end sm:justify-between">
          <div className="grid gap-1">
            <p className="text-xs text-muted-foreground">{organizationName}</p>
            <h1 className="text-xl font-semibold tracking-tight">Inbox</h1>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <label className="sr-only" htmlFor="inbox-project-filter">Filter inbox by project</label>
            <select id="inbox-project-filter" value={projectFilter} onChange={(event) => updateInboxFilters({ project: event.target.value })} className="h-11 min-w-0 max-w-full rounded-md border border-input bg-background px-2 text-xs sm:h-8">
              <option value="all">All projects</option>
              {projects.map((project) => <option key={project.id} value={project.id}>{project.label}</option>)}
            </select>
            <label className="sr-only" htmlFor="inbox-kind-filter">Filter inbox by type</label>
            <select id="inbox-kind-filter" value={kindFilter} onChange={(event) => updateInboxFilters({ kind: event.target.value })} className="h-11 min-w-0 max-w-full rounded-md border border-input bg-background px-2 text-xs sm:h-8">
              <option value="all">All types</option><option value="approval">Approvals</option><option value="assignment">Assignments</option><option value="takeover">Takeovers</option><option value="lease">Leases</option><option value="comment">Comments</option><option value="invitation">Invitations</option>
            </select>
            <span className="text-xs text-muted-foreground">{unreadCount} unread</span>
          </div>
        </header>
        {inboxQuery.isPending && notifications.length === 0 ? <p className="text-sm text-muted-foreground">Loading inbox…</p> : null}
        {inboxQuery.error ? <p className="text-sm text-destructive">{inboxQuery.error.message}</p> : null}
        {!inboxQuery.isPending && !inboxQuery.error && visibleNotifications.length === 0 ? (
          <Empty className="min-h-56 border-0">
            <EmptyHeader>
              <EmptyMedia variant="icon"><Bell /></EmptyMedia>
              <EmptyTitle>Nothing new</EmptyTitle>
            </EmptyHeader>
          </Empty>
        ) : null}
        <div className="grid gap-2">
          {visibleNotifications.map((notification) => {
            const isRead = notification.read_at !== null || readConfirmedIds.has(notification.id);
            const action = notificationAction(notification);
            const stateLabel = notificationStateLabel(notification);
            const issueLink = notification.issue_id ? <Link to="/issues/$issueId" params={{ issueId: notification.issue_id }} className="group block min-w-0 flex-1 rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-ring/50"><p className="font-medium group-hover:underline">{notificationTitle(notification.kind)}{stateLabel ? <span className="ml-2 rounded border border-border/70 px-1.5 py-0.5 text-[10px] font-normal text-muted-foreground">{stateLabel}</span> : null}</p><p className="truncate text-xs text-muted-foreground">{notificationSummary(notification)}</p></Link> : <div className="min-w-0 flex-1"><p className="font-medium">{notificationTitle(notification.kind)}{stateLabel ? <span className="ml-2 rounded border border-border/70 px-1.5 py-0.5 text-[10px] font-normal text-muted-foreground">{stateLabel}</span> : null}</p><p className="truncate text-xs text-muted-foreground">{notificationSummary(notification)}</p></div>;
            return <article key={notification.id} className={`flex flex-col gap-3 rounded-md border border-border/60 px-3 py-3 text-sm sm:flex-row sm:items-center ${isRead ? "bg-background" : "bg-muted/15"}`}>
              <span className={`size-2 shrink-0 rounded-full ${isRead ? "bg-muted" : "bg-primary"}`} />
              {issueLink}
              {action === "Review approval" ? <Link to="/$organizationSlug/approvals" params={{ organizationSlug }} className="inline-flex h-11 shrink-0 items-center rounded-md border border-border px-3 text-xs hover:bg-muted/50 sm:h-8">{action}</Link> : null}
              <time className="shrink-0 text-[10px] text-muted-foreground sm:text-right" dateTime={notification.created_at}>
                {new Date(notification.created_at).toLocaleString()}
              </time>
              {!isRead ? (
                <Button
                  aria-label="Mark notification as read"
                  size="icon"
                  variant="ghost"
                  className="size-11 self-end sm:size-8 sm:self-auto"
                  onClick={() => markReadMutation.mutate(notification.id)}
                  disabled={readPendingIds.has(notification.id)}
                >
                  {readPendingIds.has(notification.id) ? <span className="text-xs">…</span> : <Check />}
                </Button>
              ) : null}
            </article>;
          })}
        </div>
      </main>
    </ProjectShell>
  );
}
