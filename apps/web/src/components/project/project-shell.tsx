import { lazy, Suspense, useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import { useLocation, useNavigate } from "@tanstack/react-router";
import { Bell, Bot, Building2, CircleDot, ClipboardCheck, FileText, FolderKanban, Keyboard, Layers3, Settings2, ShieldAlert, UsersRound } from "lucide-react";
import { useQuery } from "@tanstack/react-query";

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import type { CommandMenuGroup } from "@/components/command/command-menu";
import type { DocumentRecord, HumanQueueIssue, NavigationResponse } from "@/lib/api";
import { organizationSlug as toOrganizationSlug } from "@/lib/organization-slug";
import { useNavigation } from "@/hooks/use-navigation";
import { advanceShortcut } from "@/lib/keyboard-shortcuts";

const LazyCommandMenu = lazy(() =>
  import("@/components/command/command-menu").then(({ CommandMenu }) => ({ default: CommandMenu })),
);
const LazyShortcutReferenceDialog = lazy(() =>
  import("@/components/command/command-menu").then(({ ShortcutReferenceDialog }) => ({ default: ShortcutReferenceDialog })),
);

export function ProjectShell({
  sidebar,
  children,
  footer,
  commandGroups = [],
  shortcuts = [],
}: {
  sidebar: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  commandGroups?: CommandMenuGroup[];
  shortcuts?: Array<{ keys: string[]; onTrigger: () => void }>;
}) {
  const location = useLocation();
  const navigate = useNavigate();
  const [globalCommandOpen, setGlobalCommandOpen] = useState(false);
  const [commandQuery, setCommandQuery] = useState("");
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const openCommandMenu = useCallback(() => setGlobalCommandOpen(true), []);
  const pathSegments = location.pathname.split("/").filter(Boolean);
  const organizationSlug = pathSegments[0] ?? "riichi";
  const teamIndex = pathSegments.indexOf("teams");
  const teamKey = teamIndex >= 0 ? pathSegments[teamIndex + 1] : undefined;
  useEffect(() => {
    const shortcutBuffer: string[] = [];
    let shortcutTimeout: number | undefined;
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        openCommandMenu();
        return;
      }
      if (globalCommandOpen || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) return;
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']")) return;
      const entries = [
        { keys: ["g", "i"], onTrigger: () => void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } }) },
        ...shortcuts,
      ];
      const result = advanceShortcut(shortcutBuffer, event.key, entries.map((entry) => entry.keys));
      shortcutBuffer.splice(0, shortcutBuffer.length, ...result.buffer);
      if (shortcutTimeout !== undefined) window.clearTimeout(shortcutTimeout);
      if (result.buffer.length > 0) {
        event.preventDefault();
        shortcutTimeout = window.setTimeout(() => shortcutBuffer.splice(0), 900);
      }
      if (result.matched) {
        event.preventDefault();
        entries.find((entry) => entry.keys.join(" ") === result.matched)?.onTrigger();
      }
    };
    const onCommandEvent = () => openCommandMenu();
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("riichi:open-command-menu", onCommandEvent);
    return () => {
      if (shortcutTimeout !== undefined) window.clearTimeout(shortcutTimeout);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("riichi:open-command-menu", onCommandEvent);
    };
  }, [globalCommandOpen, navigate, openCommandMenu, organizationSlug, shortcuts]);

  const navigationQuery = useNavigation();
  const commandSearchQuery = useQuery<{ navigation: NavigationResponse; issues: HumanQueueIssue[]; documents: DocumentRecord[] } | null>({
    queryKey: ["command-search", organizationSlug],
    enabled: globalCommandOpen && commandQuery.trim().length >= 2 && Boolean(navigationQuery.data),
    staleTime: 60_000,
    queryFn: async () => {
      const {
        getAllIssues,
        listOrganizationDocuments,
        listProjectDocuments,
        listTeamDocuments,
      } = await import("@/lib/api");
      const navigation = navigationQuery.data;
      if (!navigation) throw new Error("Navigation data is unavailable");
      const organization = navigation.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
      if (!organization) return { navigation, issues: [], documents: [] };

      const documentRequests = [
        listOrganizationDocuments(organization.id),
        ...organization.teams.flatMap((team) => [
          listTeamDocuments(team.id),
          ...team.projects.map((project) => listProjectDocuments(project.id)),
        ]),
      ];
      const [issues, documentResults] = await Promise.all([
        getAllIssues(),
        Promise.allSettled(documentRequests),
      ]);
      const documents = [
        ...new Map(
          documentResults
            .flatMap((result) => result.status === "fulfilled" ? result.value : [])
            .map((document) => [document.id, document]),
        ).values(),
      ];
      return { navigation, issues, documents };
    },
  });
  const navigationGroups = useMemo<CommandMenuGroup[]>(() => {
    const items: CommandMenuGroup["items"] = [
      {
        id: "all-issues",
        label: "All issues",
        icon: Layers3,
        shortcut: "G I",
        onSelect: () => void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } }),
      },
      {
        id: "inbox",
        label: "Inbox",
        icon: Bell,
        onSelect: () => void navigate({ to: "/$organizationSlug/inbox", params: { organizationSlug } }),
      },
      {
        id: "triage",
        label: "Triage",
        icon: ShieldAlert,
        onSelect: () => void navigate({ to: "/$organizationSlug/triage", params: { organizationSlug } }),
      },
      {
        id: "approvals",
        label: "Approvals",
        icon: ClipboardCheck,
        onSelect: () => void navigate({ to: "/$organizationSlug/approvals", params: { organizationSlug } }),
      },
      {
        id: "agents",
        label: "Agents",
        icon: Bot,
        onSelect: () => void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } }),
      },
      {
        id: "documentation",
        label: "Documentation",
        icon: FileText,
        onSelect: () => void navigate({ to: "/$organizationSlug/documents", params: { organizationSlug } }),
      },
      {
        id: "settings",
        label: "Settings",
        icon: Settings2,
        onSelect: () => void navigate({ to: "/$organizationSlug/settings", params: { organizationSlug } }),
      },
      {
        id: "shortcut-reference",
        label: "Keyboard shortcuts",
        icon: Keyboard,
        onSelect: () => setShortcutsOpen(true),
      },
    ];
    if (teamKey) {
      items.splice(1, 0, {
        id: "team-issues",
        label: `${teamKey} issues`,
        icon: FolderKanban,
        onSelect: () => void navigate({ to: "/$organizationSlug/teams/$teamKey/issues", params: { organizationSlug, teamKey } }),
      });
    }
    return [{ id: "workspace-navigation", label: "Navigate", items }];
  }, [navigate, organizationSlug, setShortcutsOpen, teamKey]);
  const searchGroups = useMemo<CommandMenuGroup[]>(() => {
    const data = commandSearchQuery.data;
    if (!data || commandQuery.trim().length < 2) return [];
    const organization = data.navigation.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
    if (!organization) return [];
    const teamsById = new Map(organization.teams.map((team) => [team.id, team]));
    const projectsById = new Map(organization.teams.flatMap((team) => team.projects).map((project) => [project.id, project]));
    const groups: CommandMenuGroup[] = [
      {
        id: "search-organizations",
        label: "Organizations",
        items: [{
          id: `organization-${organization.id}`,
          label: organization.name,
          icon: Building2,
          keywords: [organization.role],
          onSelect: () => void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } }),
        }],
      },
      {
        id: "search-teams",
        label: "Teams",
        items: organization.teams.map((team) => ({
          id: `team-${team.id}`,
          label: `${team.key} · ${team.name}`,
          icon: UsersRound,
          keywords: [team.name, team.key, team.emoji ?? ""],
          onSelect: () => void navigate({ to: "/$organizationSlug/teams/$teamKey", params: { organizationSlug, teamKey: team.key } }),
        })),
      },
      {
        id: "search-projects",
        label: "Projects",
        items: organization.teams.flatMap((team) => team.projects.map((project) => ({
          id: `project-${project.id}`,
          label: `${project.name} · ${team.key}`,
          icon: FolderKanban,
          keywords: [project.name, team.name, team.key, project.role],
          onSelect: () => void navigate({ to: "/$organizationSlug/projects/$projectId", params: { organizationSlug, projectId: project.id } }),
        }))),
      },
      {
        id: "search-documents",
        label: "Documents",
        items: data.documents.map((document) => ({
          id: `document-${document.id}`,
          label: document.title,
          icon: FileText,
          keywords: [
            document.kind,
            document.plain_text ?? "",
            document.owner_team_id ? teamsById.get(document.owner_team_id)?.name ?? "" : "",
            document.owner_team_id ? teamsById.get(document.owner_team_id)?.key ?? "" : "",
            document.owner_project_id ? projectsById.get(document.owner_project_id)?.name ?? "" : "",
          ],
          onSelect: () => void navigate({ to: "/$organizationSlug/documents/$documentId", params: { organizationSlug, documentId: document.id } }),
        })),
      },
      {
        id: "search-issues",
        label: "Issues",
        items: data.issues.map((issue) => ({
          id: `issue-${issue.id}`,
          label: `${issue.display_key} · ${issue.title}`,
          icon: CircleDot,
          keywords: [
            issue.title,
            issue.body,
            issue.team_name,
            issue.team_key,
            issue.project_name,
            issue.status,
            issue.importance,
            ...issue.labels,
          ],
          onSelect: () => void navigate({ to: "/$organizationSlug/teams/$teamKey/issues/$issueId", params: { organizationSlug, teamKey: issue.team_key, issueId: issue.id } }),
        })),
      },
    ];
    return groups.filter((group) => group.items.length > 0);
  }, [commandQuery, commandSearchQuery.data, navigate, organizationSlug]);
  const globalCommandGroups = useMemo(
    () => [...navigationGroups, ...searchGroups, ...commandGroups],
    [commandGroups, navigationGroups, searchGroups],
  );

  return (
    <div className="min-h-svh bg-background">
      <SidebarProvider
        defaultOpen
        className="app-frame min-h-svh w-full overflow-hidden bg-background"
      >
        {sidebar}
        <SidebarInset className="min-h-0 overflow-hidden bg-background">
          {children}
          {footer}
        </SidebarInset>
        {globalCommandOpen ? (
          <Suspense fallback={null}>
            <LazyCommandMenu
              open={globalCommandOpen}
              onOpenChange={(open) => {
                setGlobalCommandOpen(open);
                if (!open) setCommandQuery("");
              }}
              groups={globalCommandGroups}
              onSearchChange={setCommandQuery}
              placeholder="Search everything..."
              description="Search issues, projects, teams, documents, and actions."
            />
          </Suspense>
        ) : null}
        {shortcutsOpen ? (
          <Suspense fallback={null}>
            <LazyShortcutReferenceDialog open={shortcutsOpen} onOpenChange={setShortcutsOpen} />
          </Suspense>
        ) : null}
      </SidebarProvider>
    </div>
  );
}
