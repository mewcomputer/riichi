import { useMutation, useQuery } from "@tanstack/react-query";
import { useNavigate, useParams } from "@tanstack/react-router";

import { DocumentTree } from "@/components/documents/document-tree";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { Button } from "@/components/ui/button";
import { createOrganizationDocument, getCurrentUser, listOrganizationDocuments } from "@/lib/api";
import { useActiveProject } from "@/hooks/use-active-project";
import { useAppLogout } from "@/hooks/use-app-logout";
import { useHumanDocuments } from "@/hooks/use-human-documents";
import { useNavigation } from "@/hooks/use-navigation";
import { organizationSlug as toOrganizationSlug } from "@/lib/organization-slug";

export function OrganizationDocumentsPage() {
  const { organizationSlug } = useParams({ from: "/$organizationSlug/documents" });
  const navigate = useNavigate();
  const appLogout = useAppLogout();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const syncedDocuments = useHumanDocuments();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const organization = navigationQuery.data?.organizations.find(
    (candidate) => toOrganizationSlug(candidate.name) === organizationSlug,
  );
  const documentsQuery = useQuery({
    queryKey: ["organization-documents", organization?.id],
    queryFn: () => listOrganizationDocuments(organization!.id),
    enabled: Boolean(organization),
  });
  const createDocumentMutation = useMutation({
    mutationFn: (parentDocumentId?: string) => createOrganizationDocument(organization!.id, {
      title: "Untitled document",
      parent_document_id: parentDocumentId,
    }),
    onSuccess: (document) => void navigate({
      to: "/$organizationSlug/documents/$documentId",
      params: { organizationSlug, documentId: document.id },
    }),
  });
  const organizationDocuments = syncedDocuments?.filter((document) =>
    document.owner_team_id === null && document.owner_project_id === null,
  ) ?? documentsQuery.data;

  if (!organization) return <div className="p-8 text-sm text-muted-foreground">Loading documentation…</div>;

  return (
    <ProjectShell
      sidebar={
        <ProjectSidebar
          projectName={activeMembership?.project_name ?? organization.name}
          navigation={navigationQuery.data}
          memberships={meQuery.data?.memberships}
          activeProjectId={projectId}
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
        content={<div className="flex min-w-0 items-center gap-2 text-xs"><span className="truncate font-medium">{organization.name}</span><span className="text-muted-foreground/50">›</span><span className="truncate text-muted-foreground">Documentation</span></div>}
      />
      <main className="mx-auto flex w-full max-w-screen-lg flex-col gap-8 px-8 py-10">
        <header className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-2xl font-medium tracking-tight">Documentation</h1>
            <p className="mt-2 max-w-xl text-sm text-muted-foreground">
              Shared notes, specifications, and runbooks for {organization.name}.
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => createDocumentMutation.mutate(undefined)}
            disabled={createDocumentMutation.isPending}
          >
            New page
          </Button>
        </header>
        {createDocumentMutation.error ? (
          <p role="alert" className="text-xs text-destructive">
            Could not create the page: {createDocumentMutation.error.message}
          </p>
        ) : null}
        {organizationDocuments?.length ? (
          <DocumentTree
            organizationSlug={organizationSlug}
            documents={organizationDocuments}
            listChildren={(parentDocumentId) => syncedDocuments
              ? Promise.resolve(organizationDocuments.filter((document) => document.parent_document_id === (parentDocumentId ?? null)))
              : listOrganizationDocuments(organization.id, parentDocumentId)}
            onCreate={(parentDocumentId) => createDocumentMutation.mutate(parentDocumentId)}
          />
        ) : (
          <div className="rounded-md border border-dashed border-border/70 px-4 py-8 text-sm text-muted-foreground">
            No organization pages yet.
          </div>
        )}
      </main>
    </ProjectShell>
  );
}
