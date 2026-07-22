import {
  ChevronDown,
  Folder,
  Layers3,
  ListFilter,
  ClipboardCheck,
  AlertTriangle,
  Bell,
  Plus,
  Search,
  Settings2,
  MoreHorizontal,
  Users,
  FileText,
  type LucideIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarSeparator,
} from "@/components/ui/sidebar";
import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import type { HumanMembership, NavigationResponse, SavedView } from "@/lib/api";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { organizationSlug as toOrganizationSlug } from "@/lib/organization-slug";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";

export type ProjectNavItem = {
  label: string;
  icon: LucideIcon;
  to?: string;
  active?: boolean;
  badge?: string;
};

export type ProjectNavSection = {
  label?: string;
  items: ProjectNavItem[];
};

export const defaultProjectNavSections: ProjectNavSection[] = [
  {
    items: [
      { label: "Inbox", icon: Bell, to: "/inbox" },
      { label: "Issues", icon: Layers3, to: "/" },
      { label: "Triage", icon: AlertTriangle, to: "/triage" },
      { label: "Approvals", icon: ClipboardCheck, to: "/approvals" },
    ],
  },
  {
    label: "Project",
    items: [
      { label: "Documentation", icon: FileText, to: "/documents" },
      { label: "Agents", icon: Users, to: "/agents" },
      { label: "Settings", icon: Settings2, to: "/settings" },
    ],
  },
];

function NavigationItem({ label, icon: Icon, active, badge, onClick, isCurrent }: ProjectNavItem & { onClick?: () => void; isCurrent?: boolean }) {
  return (
    <SidebarMenuItem>
      <SidebarMenuButton isActive={active || isCurrent} tooltip={label} onClick={onClick} className="transition-[background-color,transform] duration-100 active:scale-[0.985]">
        <Icon />
        <span>{label}</span>
        {badge ? <SidebarMenuBadge>{badge}</SidebarMenuBadge> : null}
      </SidebarMenuButton>
    </SidebarMenuItem>
  );
}

function TeamNavigationSection({
  team,
  organizationSlug,
  activeProjectId,
  onProjectChange,
  onNavigate,
}: {
  team: NavigationResponse["organizations"][number]["teams"][number];
  organizationSlug: string;
  activeProjectId?: string;
  onProjectChange?: (projectId: string) => void;
  onNavigate?: (label: string) => void;
}) {
  const [open, setOpen] = useState(true);
  const [projectsOpen, setProjectsOpen] = useState(true);
  const navigate = useNavigate();

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <SidebarGroup className="-mt-2">
        <div className="flex h-8 min-w-0 items-center gap-1 px-2 text-xs font-medium text-sidebar-foreground/70">
          <button type="button" className="flex min-w-0 flex-1 items-center gap-2 truncate text-left hover:text-sidebar-foreground" onClick={() => void navigate({ to: "/$organizationSlug/teams/$teamKey", params: { organizationSlug, teamKey: team.key } })}>
            {team.emoji ? <span className="text-sm">{team.emoji}</span> : null}
            <span className="truncate">{team.name}</span>
            <span className="ml-auto mr-1 shrink-0 font-normal text-sidebar-foreground/45 font-mono">{team.key}</span>
          </button>
          <DropdownMenu>
            <DropdownMenuTrigger render={<Button variant="ghost" size="icon-xs" className="shrink-0 text-sidebar-foreground/50 hover:text-sidebar-foreground" aria-label={`${team.name} menu`} />}>
              <MoreHorizontal />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-40">
              <DropdownMenuItem onClick={() => void navigate({ to: "/$organizationSlug/teams/$teamKey/settings", params: { organizationSlug, teamKey: team.key } })}>
                <Settings2 /> Team settings
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <CollapsibleTrigger render={<Button variant="ghost" size="icon-xs" className="shrink-0 text-sidebar-foreground/50 hover:text-sidebar-foreground" aria-label={`${open ? "Collapse" : "Expand"} ${team.name}`} />}>
            <ChevronDown className={`size-3 transition-transform ${open ? "" : "-rotate-90"}`} />
          </CollapsibleTrigger>
        </div>
        <CollapsibleContent>
          <SidebarGroupContent>
            <SidebarMenu>
              <NavigationItem label="Issues" icon={Layers3} onClick={() => void navigate({ to: "/$organizationSlug/teams/$teamKey/issues", params: { organizationSlug, teamKey: team.key } })} />
              <Collapsible open={projectsOpen} onOpenChange={setProjectsOpen}>
                <SidebarMenuItem>
                  <CollapsibleTrigger
                    render={<SidebarMenuButton tooltip="Projects" onClick={() => onNavigate?.("Projects")} />}
                  >
                    <Folder />
                    <span>Projects</span>
                    <ChevronDown className={`ml-auto size-3 transition-transform ${projectsOpen ? "" : "-rotate-90"}`} />
                  </CollapsibleTrigger>
                  <CollapsibleContent>
                    <SidebarMenuSub>
                      {team.projects.map((project) => (
                        <SidebarMenuSubItem key={project.id}>
                          <SidebarMenuSubButton
                            isActive={project.id === activeProjectId}
                            onClick={() => {
                              onProjectChange?.(project.id);
                              void navigate({ to: "/$organizationSlug/projects/$projectId", params: { organizationSlug, projectId: project.id } });
                            }}
                          >
                            <Folder />
                            <span>{project.name}</span>
                          </SidebarMenuSubButton>
                        </SidebarMenuSubItem>
                      ))}
                    </SidebarMenuSub>
                  </CollapsibleContent>
                </SidebarMenuItem>
              </Collapsible>
              <NavigationItem label="Views" icon={ListFilter} onClick={() => onNavigate?.("Views")} />
            </SidebarMenu>
          </SidebarGroupContent>
        </CollapsibleContent>
      </SidebarGroup>
    </Collapsible>
  );
}

export function ProjectSidebar({
  onSearch,
  onCreate,
  projectName = "riichi",
  projectMark = "R",
  userName = "Alex Morgan",
  avatarUrl,
  navSections = defaultProjectNavSections,
  onNavigate,
  onLogout,
  memberships,
  navigation,
  activeProjectId,
  onProjectChange,
  pinnedViews = [],
  onPinnedViewSelect,
}: {
  onSearch?: () => void;
  onCreate?: () => void;
  projectName?: string;
  projectMark?: string;
  userName?: string;
  avatarUrl?: string | null;
  navSections?: ProjectNavSection[];
  onNavigate?: (label: string) => void;
  onLogout?: () => void;
  memberships?: HumanMembership[];
  navigation?: NavigationResponse;
  activeProjectId?: string;
  onProjectChange?: (projectId: string) => void;
  pinnedViews?: SavedView[];
  onPinnedViewSelect?: (view: SavedView) => void;
}) {
  const navigate = useNavigate();
  const currentPath = typeof window === "undefined" ? "" : window.location.pathname;
  const organizationName = navigation?.organizations[0]?.name ?? projectName;
  const organizationSlug = toOrganizationSlug(organizationName);
  const organizationLogoUrl = navigation?.organizations[0]?.logo_url;
  const navigateItem = (item: ProjectNavItem) => {
    onNavigate?.(item.label);
    if (item.to === "/") void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } });
    if (item.to === "/agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
    if (item.to === "/documents") void navigate({ to: "/$organizationSlug/documents", params: { organizationSlug } });
    if (item.to === "/settings") void navigate({ to: "/$organizationSlug/settings", params: { organizationSlug } });
    if (item.to === "/approvals") void navigate({ to: "/$organizationSlug/approvals", params: { organizationSlug } });
    if (item.to === "/triage") void navigate({ to: "/$organizationSlug/triage", params: { organizationSlug } });
    if (item.to === "/inbox") void navigate({ to: "/$organizationSlug/inbox", params: { organizationSlug } });
  };
  return (
    <Sidebar collapsible="offcanvas" variant="inset">
      <SidebarHeader className="gap-2 border-b border-sidebar-border/70 p-2">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton className="h-9 hover:bg-sidebar-accent" tooltip={`${organizationName} organization`}>
                {organizationLogoUrl ? <img src={organizationLogoUrl} alt="" className="size-6 shrink-0 rounded-md object-cover" /> : <span className="grid size-6 shrink-0 place-items-center rounded-md bg-foreground/90 text-xs font-semibold text-background">{projectMark}</span>}
                <span className="truncate text-sm font-semibold">{organizationName}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        <div className="flex items-center gap-1 px-1">
          <Button
            variant="ghost"
            size="sm"
            className="h-7 flex-1 justify-start gap-2 px-2 text-xs text-sidebar-foreground/60 hover:text-sidebar-foreground"
            onClick={() => {
              if (onSearch) {
                onSearch();
              } else {
                window.dispatchEvent(new Event("riichi:open-command-menu"));
              }
            }}
          >
            <Search className="size-3.5" /> Search
            <Kbd className="ml-auto h-5 bg-sidebar-accent px-1.5 text-[10px] text-sidebar-foreground/55">
              ⌘ K
            </Kbd>
          </Button>
          <Button
            variant="ghost"
            size="icon-sm"
            className="text-sidebar-foreground/60"
            aria-label="Create"
            onClick={onCreate}
          >
            <Plus />
          </Button>
        </div>
      </SidebarHeader>
      <SidebarContent>
        {navSections.map((section, index) => (
          <SidebarGroup
            key={section.label ?? `section-${index}`}
            className="py-2"
          >
            {section.label ? (
              <SidebarGroupLabel>
                {section.label}
                <ChevronDown className="ml-0.5 size-3" />
              </SidebarGroupLabel>
            ) : null}
            <SidebarGroupContent>
              <SidebarMenu>
                {section.items.map((item) => (
                  <NavigationItem key={item.label} {...item} isCurrent={item.to === "/" ? currentPath === `/${organizationSlug}/issues` : item.to ? currentPath === `/${organizationSlug}${item.to}` : false} onClick={() => navigateItem(item)} />
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
        {pinnedViews.length > 0 ? <SidebarGroup className="py-2">
          <SidebarGroupLabel>Pinned views</SidebarGroupLabel>
          <SidebarGroupContent><SidebarMenu>{pinnedViews.map((view) => <NavigationItem key={view.id} label={view.name} icon={ListFilter} onClick={() => onPinnedViewSelect?.(view)} />)}</SidebarMenu></SidebarGroupContent>
        </SidebarGroup> : null}
        {(() => {
          const teams = (navigation?.organizations ?? []).flatMap((organization) => organization.teams);
          if (teams.length === 0) return null;
          return (
            <>
              <SidebarGroup className="pb-0">
                <SidebarGroupLabel className="py-0">Your teams</SidebarGroupLabel>
                {teams.map((team) => (
                <TeamNavigationSection key={team.id} team={team} organizationSlug={organizationSlug} activeProjectId={activeProjectId} onProjectChange={onProjectChange} onNavigate={onNavigate} />
                ))}
              </SidebarGroup>
            </>
          );
        })()}
      </SidebarContent>
      <SidebarSeparator />
      <SidebarFooter className="p-2">
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton tooltip="Open personal settings" onClick={() => void navigate({ to: "/$organizationSlug/settings", params: { organizationSlug } })}>
              <Avatar key={avatarUrl ?? "fallback"} size="sm" className="animate-in zoom-in-95 duration-200">
                {avatarUrl ? <AvatarImage src={avatarUrl} alt="" /> : null}
                <AvatarFallback>
                  {userName.split(" ").map((part) => part[0]).join("")}
                </AvatarFallback>
              </Avatar>
              <span className="truncate">{userName}</span>
              <Settings2 className="ml-auto size-3.5 text-sidebar-foreground/50" />
            </SidebarMenuButton>
          </SidebarMenuItem>
          <SidebarMenuItem>
            <SidebarMenuButton tooltip="Sign out" onClick={onLogout}>
              <span className="text-xs text-sidebar-foreground/60">Sign out</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}
