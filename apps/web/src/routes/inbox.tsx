import { useMemo } from "react";
import { useLiveQuery } from "@tanstack/react-db";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";
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

export function InboxPage() {
  const { organizationSlug } = useParams({ from: "/$organizationSlug/inbox" });
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const queryClient = useQueryClient();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const collection = useMemo(() => createNotificationCollection(), []);
  const liveQuery = useLiveQuery(() => collection, [collection]);
  const inboxQuery = useQuery({ queryKey: ["inbox"], queryFn: () => getInbox({ limit: 100 }) });
  const replicatedNotifications = liveQuery.data as Notification[] | undefined;
  const notifications: Notification[] = replicatedNotifications ?? (inboxQuery.data?.notifications ?? []).map((notification) => ({
    ...notification,
    recipient_account_id: notification.recipient_account_id ?? "",
    project_id: notification.project_id ?? null,
    issue_id: notification.issue_id ?? null,
    actor_id: notification.actor_id ?? null,
    read_at: notification.read_at ?? null,
  }));
  const unreadCount = notifications.filter((notification) => notification.read_at === null).length;
  const markReadMutation = useMutation({
    mutationFn: markInboxNotificationRead,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["inbox"] }),
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
      <main className="mx-auto w-full max-w-4xl px-8 py-8">
        <header className="mb-6 flex items-end justify-between gap-4 border-b border-border/60 pb-5">
          <div className="grid gap-1">
            <p className="text-xs text-muted-foreground">{organizationName}</p>
            <h1 className="text-xl font-semibold tracking-tight">Inbox</h1>
          </div>
          <span className="text-xs text-muted-foreground">{unreadCount} unread</span>
        </header>
        {inboxQuery.isPending && notifications.length === 0 ? <p className="text-sm text-muted-foreground">Loading inbox…</p> : null}
        {inboxQuery.error ? <p className="text-sm text-destructive">{inboxQuery.error.message}</p> : null}
        {!inboxQuery.isPending && !inboxQuery.error && notifications.length === 0 ? (
          <Empty className="min-h-56 border-0">
            <EmptyHeader>
              <EmptyMedia variant="icon"><Bell /></EmptyMedia>
              <EmptyTitle>Nothing new</EmptyTitle>
            </EmptyHeader>
          </Empty>
        ) : null}
        <div className="grid gap-1">
          {notifications.map((notification) => (
            <article key={notification.id} className="flex items-center gap-3 rounded-md border border-border/60 px-3 py-3 text-sm">
              <span className={`size-2 shrink-0 rounded-full ${notification.read_at ? "bg-muted" : "bg-primary"}`} />
              <div className="min-w-0 flex-1">
                <p className="font-medium capitalize">{notification.kind.replaceAll("_", " ")}</p>
                <p className="truncate text-xs text-muted-foreground">
                  {typeof notification.payload.body === "string" ? notification.payload.body : "You have a new Riichi notification."}
                </p>
              </div>
              <time className="shrink-0 text-[10px] text-muted-foreground" dateTime={notification.created_at}>
                {new Date(notification.created_at).toLocaleString()}
              </time>
              {!notification.read_at ? (
                <Button
                  aria-label="Mark notification as read"
                  size="icon"
                  variant="ghost"
                  className="size-7"
                  onClick={() => markReadMutation.mutate(notification.id)}
                  disabled={markReadMutation.isPending}
                >
                  <Check />
                </Button>
              ) : null}
            </article>
          ))}
        </div>
      </main>
    </ProjectShell>
  );
}
