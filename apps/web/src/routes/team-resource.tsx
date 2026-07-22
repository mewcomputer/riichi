import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import { DocumentTree } from "@/components/documents/document-tree";
import { createTeamDocument, getCurrentUser, listTeamDocuments } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";
import { useHumanDocuments } from "../hooks/use-human-documents";
import { TeamMark } from "@/components/team/team-mark";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";

export function TeamResourcePage() {
  const { organizationSlug, teamKey } = useParams({ from: "/$organizationSlug/teams/$teamKey" });
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const syncedDocuments = useHumanDocuments();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const organization = navigationQuery.data?.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
  const team = organization?.teams.find((candidate) => candidate.key.toLowerCase() === teamKey.toLowerCase());
  const documentsQuery = useQuery({
    queryKey: ["team-documents", team?.id],
    queryFn: () => listTeamDocuments(team!.id),
    enabled: Boolean(team),
  });
  const createDocumentMutation = useMutation({
    mutationFn: (parentDocumentId?: string) => createTeamDocument(team!.id, { title: "Untitled document", parent_document_id: parentDocumentId }),
    onSuccess: (document) => void navigate({
      to: "/$organizationSlug/documents/$documentId",
      params: { organizationSlug, documentId: document.id },
    }),
  });
  const teamDocuments = syncedDocuments?.filter((document) =>
    document.owner_team_id === team?.id && document.owner_project_id === null,
  ) ?? documentsQuery.data;

  if (!team) return <div className="p-8 text-sm text-muted-foreground">Loading team…</div>;
  return (
    <ProjectShell
      sidebar={
        <ProjectSidebar
          projectName={activeMembership?.project_name ?? "riichi"}
          navigation={navigationQuery.data}
          memberships={meQuery.data?.memberships}
          activeProjectId={projectId}
          onProjectChange={selectProject}
          onLogout={appLogout}
          avatarUrl={meQuery.data?.avatar_url}
          onNavigate={(label) => {
            if (label === "Issues") void navigate({ to: "/$organizationSlug/teams/$teamKey/issues", params: { organizationSlug, teamKey: team.key } });
            if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
          }}
          userName={meQuery.data?.display_name ?? "Alex Morgan"}
        />
      }
    >
      <ProjectHeader
        view="all"
        views={[]}
        onViewChange={() => undefined}
          content={<div className="flex min-w-0 items-center gap-2 text-xs"><TeamMark value={team.emoji} className="size-3.5" /><span className="truncate font-medium">{team.name}</span><span className="font-mono text-muted-foreground">{team.key}</span></div>}
      />
      <main className="mx-auto flex w-full max-w-screen-lg flex-col gap-8 px-8 py-10">
        <header className="grid gap-2">
          <div className="flex items-center gap-3"><span className="grid size-10 place-items-center rounded-xl border border-border/70 bg-card text-xl"><TeamMark value={team.emoji} className="size-5" /></span><div><h1 className="text-2xl font-medium tracking-tight">{team.name}</h1><p className="font-mono text-xs text-muted-foreground">{team.key}</p></div></div>
          <p className="max-w-xl text-sm text-muted-foreground">A shared home for this team’s issues, projects, and future documentation.</p>
        </header>
        <section className="grid gap-3 border-y border-border/60 py-6">
          <div className="flex items-start justify-between gap-4">
            <div><h2 className="text-sm font-medium">Documentation</h2><p className="mt-1 text-xs text-muted-foreground">Shared notes and runbooks for {team.name}.</p></div>
            <Button type="button" variant="outline" size="sm" onClick={() => createDocumentMutation.mutate(undefined)} disabled={createDocumentMutation.isPending}>New page</Button>
          </div>
          {createDocumentMutation.error ? (
            <p role="alert" className="text-xs text-destructive">
              Could not create the page: {createDocumentMutation.error.message}
            </p>
          ) : null}
          {teamDocuments?.length ? (
            <DocumentTree
              organizationSlug={organizationSlug}
              documents={teamDocuments}
              listChildren={(parentDocumentId) => syncedDocuments
                ? Promise.resolve(teamDocuments.filter((document) => document.parent_document_id === (parentDocumentId ?? null)))
                : listTeamDocuments(team.id, parentDocumentId)}
              onCreate={(parentDocumentId) => createDocumentMutation.mutate(parentDocumentId)}
            />
          ) : <div className="rounded-md border border-dashed border-border/70 px-4 py-5 text-xs text-muted-foreground">No documents yet.</div>}
        </section>
        <section className="grid gap-3">
          <div><h2 className="text-sm font-medium">Projects</h2><p className="mt-1 text-xs text-muted-foreground">Projects connected to {team.name}.</p></div>
          <div className="grid gap-2">{team.projects.map((project) => <Link key={project.id} to="/$organizationSlug/projects/$projectId" params={{ organizationSlug, projectId: project.id }} onClick={() => selectProject(project.id)} className="flex items-center justify-between rounded-md border border-border/60 px-3 py-2 text-sm hover:bg-muted/30"><span>{project.name}</span><span className="text-xs text-muted-foreground">{project.role}</span></Link>)}</div>
        </section>
        <div><Link to="/$organizationSlug/teams/$teamKey/settings" params={{ organizationSlug, teamKey: team.key }} className="text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline">Open team settings</Link></div>
      </main>
    </ProjectShell>
  );
}
