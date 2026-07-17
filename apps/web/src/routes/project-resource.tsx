import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import { DocumentTree } from "@/components/documents/document-tree";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { Button } from "@/components/ui/button";
import { createProjectDocument, getCurrentUser, listProjectDocuments } from "@/lib/api";
import { useActiveProject } from "@/hooks/use-active-project";
import { useAppLogout } from "@/hooks/use-app-logout";
import { useNavigation } from "@/hooks/use-navigation";
import { useHumanDocuments } from "@/hooks/use-human-documents";
import { organizationSlug as toOrganizationSlug } from "@/lib/organization-slug";

export function ProjectResourcePage() {
  const { organizationSlug, projectId } = useParams({ from: "/$organizationSlug/projects/$projectId" });
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const syncedDocuments = useHumanDocuments();
  const { activeMembership, selectProject } = useActiveProject(meQuery.data?.memberships);
  const organization = navigationQuery.data?.organizations.find((candidate) => toOrganizationSlug(candidate.name) === organizationSlug);
  const projectScope = organization?.teams
    .flatMap((team) => team.projects.map((project) => ({ project, team })))
    .find(({ project }) => project.id === projectId);
  const project = projectScope?.project;
  const team = projectScope?.team;
  const documentsQuery = useQuery({
    queryKey: ["project-documents", project?.id],
    queryFn: () => listProjectDocuments(project!.id),
    enabled: Boolean(project),
  });
  const createDocumentMutation = useMutation({
    mutationFn: (parentDocumentId?: string) => createProjectDocument(project!.id, { title: "Untitled document", parent_document_id: parentDocumentId }),
    onSuccess: (document) => void navigate({
      to: "/$organizationSlug/documents/$documentId",
      params: { organizationSlug, documentId: document.id },
    }),
  });
  const projectDocuments = syncedDocuments?.filter((document) => document.owner_project_id === project?.id) ?? documentsQuery.data;

  if (!project || !team) return <div className="p-8 text-sm text-muted-foreground">Loading project…</div>;

  return (
    <ProjectShell
      sidebar={
        <ProjectSidebar
          projectName={project.name}
          navigation={navigationQuery.data}
          memberships={meQuery.data?.memberships}
          activeProjectId={project.id}
          onProjectChange={selectProject}
          onLogout={appLogout}
          avatarUrl={meQuery.data?.avatar_url}
          onNavigate={(label) => {
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
        content={
          <div className="flex min-w-0 items-center gap-2 text-xs">
            <Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="text-muted-foreground hover:text-foreground">
              {organization?.name ?? organizationSlug}
            </Link>
            <span className="text-muted-foreground/40">›</span>
            <Link to="/$organizationSlug/teams/$teamKey" params={{ organizationSlug, teamKey: team.key }} className="text-muted-foreground hover:text-foreground">
              {team.name}
            </Link>
            <span className="text-muted-foreground/40">›</span>
            <span className="truncate font-medium">{project.name}</span>
          </div>
        }
      />
      <main className="mx-auto flex w-full max-w-screen-lg flex-col gap-8 px-8 py-10">
        <header className="grid gap-2">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="font-mono text-xs text-muted-foreground">{team.key} project</p>
              <h1 className="mt-1 text-2xl font-medium tracking-tight">{project.name}</h1>
            </div>
            <Link
              to="/$organizationSlug/teams/$teamKey/issues"
              params={{ organizationSlug, teamKey: team.key }}
              onClick={() => selectProject(project.id)}
              className="text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
            >
              Open issues
            </Link>
          </div>
          <p className="max-w-xl text-sm text-muted-foreground">Project documentation, notes, and working context for {project.name}.</p>
          <p className="text-xs text-muted-foreground">Your role: {project.role}{activeMembership?.project_id === project.id ? " · active project" : ""}</p>
        </header>
        <section className="grid gap-3 border-y border-border/60 py-6">
          <div className="flex items-start justify-between gap-4">
            <div>
              <h2 className="text-sm font-medium">Documentation</h2>
              <p className="mt-1 text-xs text-muted-foreground">Nested pages for this project’s specifications and notes.</p>
            </div>
            <Button type="button" variant="outline" size="sm" onClick={() => createDocumentMutation.mutate(undefined)} disabled={createDocumentMutation.isPending}>New page</Button>
          </div>
          {createDocumentMutation.error ? (
            <p role="alert" className="text-xs text-destructive">
              Could not create the page: {createDocumentMutation.error.message}
            </p>
          ) : null}
          {projectDocuments?.length ? (
            <DocumentTree
              organizationSlug={organizationSlug}
              documents={projectDocuments}
              listChildren={(parentDocumentId) => syncedDocuments
                ? Promise.resolve(projectDocuments.filter((document) => document.parent_document_id === (parentDocumentId ?? null)))
                : listProjectDocuments(project.id, parentDocumentId)}
              onCreate={(parentDocumentId) => createDocumentMutation.mutate(parentDocumentId)}
            />
          ) : <div className="rounded-md border border-dashed border-border/70 px-4 py-5 text-xs text-muted-foreground">No documents yet.</div>}
        </section>
      </main>
    </ProjectShell>
  );
}
