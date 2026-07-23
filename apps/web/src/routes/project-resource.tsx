import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import { DocumentTree } from "@/components/documents/document-tree";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { ProjectIconEditor } from "@/components/project/project-icon-editor";
import { ProjectMark } from "@/components/project/project-mark";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { createProjectDocument, deleteProject, getCurrentUser, getGithubPullRequests, getProjectOverview, listProjectDocuments } from "@/lib/api";
import { useActiveProject } from "@/hooks/use-active-project";
import { useAppLogout } from "@/hooks/use-app-logout";
import { useNavigation } from "@/hooks/use-navigation";
import { useHumanDocuments } from "@/hooks/use-human-documents";
import { organizationSlug as toOrganizationSlug } from "@/lib/organization-slug";

export function ProjectResourcePage() {
  const { organizationSlug, projectId } = useParams({ from: "/$organizationSlug/projects/$projectId" });
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const appLogout = useAppLogout();
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deleteTeamName, setDeleteTeamName] = useState("");
  const [deleteProjectName, setDeleteProjectName] = useState("");
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
  const overviewQuery = useQuery({
    queryKey: ["project-overview", project?.id],
    queryFn: () => getProjectOverview(project!.id),
    enabled: Boolean(project),
  });
  const githubPullRequestsQuery = useQuery({
    queryKey: ["github-pull-requests", project?.id],
    queryFn: () => getGithubPullRequests(project!.id),
    enabled: Boolean(project),
  });
  const createDocumentMutation = useMutation({
    mutationFn: (parentDocumentId?: string) => createProjectDocument(project!.id, { title: "Untitled document", parent_document_id: parentDocumentId }),
    onSuccess: (document) => void navigate({
      to: "/$organizationSlug/documents/$documentId",
      params: { organizationSlug, documentId: document.id },
    }),
  });
  const deleteMutation = useMutation({
    mutationFn: () => deleteProject(project!.id, { team_name: deleteTeamName, project_name: deleteProjectName }),
    onSuccess: async () => {
      window.localStorage.removeItem("riichi.activeProjectId");
      await queryClient.invalidateQueries({ queryKey: ["navigation"] });
      setDeleteDialogOpen(false);
      void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } });
    },
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
              <div className="mt-1 flex items-center gap-2"><ProjectMark value={project.icon} className="size-6 text-muted-foreground" /><h1 className="text-2xl font-medium tracking-tight">{project.name}</h1></div>
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
          <div className="flex items-center justify-between gap-4"><p className="text-xs text-muted-foreground">Your role: {project.role}{activeMembership?.project_id === project.id ? " · active project" : ""}</p><div className="flex items-center gap-2"><ProjectIconEditor project={project} canManage={project.role === "owner" || project.role === "admin"} onSaved={() => void queryClient.invalidateQueries({ queryKey: ["navigation"] })} />{project.role === "owner" || project.role === "admin" ? <Button type="button" variant="destructive" size="sm" onClick={() => { deleteMutation.reset(); setDeleteTeamName(""); setDeleteProjectName(""); setDeleteDialogOpen(true); }}>Delete project</Button> : null}</div></div>
          {deleteMutation.error ? <p role="alert" className="text-xs text-destructive">Could not delete the project: {deleteMutation.error.message}</p> : null}
        </header>
        <section className="grid gap-4 border-y border-border/60 py-6">
          <div><h2 className="text-sm font-medium">Operational overview</h2><p className="mt-1 text-xs text-muted-foreground">Observable project state from issues, leases, holds, approvals, and recent activity.</p></div>
          {overviewQuery.isPending ? <p className="text-xs text-muted-foreground">Loading project state…</p> : null}
          {overviewQuery.error ? <p role="alert" className="text-xs text-destructive">Could not load the overview: {overviewQuery.error.message}</p> : null}
          {overviewQuery.data ? <>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-5">
              {[["Moving", overviewQuery.data.summary.moving_count], ["Blocked", overviewQuery.data.summary.blocked_count], ["Needs human", overviewQuery.data.summary.needs_human_count], ["Agent-owned", overviewQuery.data.summary.agent_handling_count], ["Due soon", overviewQuery.data.summary.due_soon_count]].map(([label, count]) => <div key={label} className="rounded-md border border-border/60 bg-card/20 p-3"><p className="text-[11px] text-muted-foreground">{label}</p><p className="mt-1 text-xl font-medium tabular-nums">{count}</p></div>)}
            </div>
            {overviewQuery.data.issues_truncated ? <p className="text-xs text-muted-foreground">Showing the first 200 issues. Open issues to view the complete project queue.</p> : null}
            <div className="grid gap-4 lg:grid-cols-2">
              {(["blocked", "needs_human", "agent_handling", "unowned", "due_soon"] as const).map((category) => {
                const items = overviewQuery.data!.issues.filter((issue) => issue.category === category).slice(0, 5);
                if (!items.length) return null;
                const label = category.replace("_", " ");
                return <div key={category} className="grid gap-2"><h3 className="text-xs font-medium capitalize text-muted-foreground">{label}</h3>{items.map((issue) => <Link key={issue.id} to="/$organizationSlug/teams/$teamKey/issues/$issueId" params={{ organizationSlug, teamKey: team.key, issueId: issue.id }} className="rounded-md border border-border/60 px-3 py-2 text-xs hover:bg-muted/30"><div className="flex items-center justify-between gap-3"><span className="font-mono text-muted-foreground">{issue.display_key}</span><span className="capitalize text-muted-foreground">{issue.status.replaceAll("_", " ")}</span></div><p className="mt-1 truncate">{issue.title}</p></Link>)}</div>;
              })}
            </div>
            {overviewQuery.data.recent_changes.length ? <div className="grid gap-2"><h3 className="text-xs font-medium text-muted-foreground">Recent changes</h3>{overviewQuery.data.recent_changes.slice(0, 8).map((change) => <div key={change.id} className="flex items-center justify-between gap-3 text-xs"><span className="truncate">{change.operation.replaceAll("_", " ")} {change.issue_display_key ? `· ${change.issue_display_key}` : ""}</span><time className="shrink-0 text-muted-foreground" dateTime={change.created_at}>{new Date(change.created_at).toLocaleDateString()}</time></div>)}</div> : null}
          </> : null}
        </section>
        {githubPullRequestsQuery.data?.pull_requests.length ? <section className="grid gap-3 border-y border-border/60 py-6"><div><h2 className="text-sm font-medium">GitHub pull requests</h2><p className="mt-1 text-xs text-muted-foreground">Read-only review and CI state from the configured GitHub integration.</p></div><div className="grid gap-2">{githubPullRequestsQuery.data.pull_requests.slice(0, 10).map((pull) => { const headSha = typeof pull.payload.pull_request === "object" && pull.payload.pull_request && "head" in pull.payload.pull_request && typeof pull.payload.pull_request.head === "object" && pull.payload.pull_request.head && "sha" in pull.payload.pull_request.head && typeof pull.payload.pull_request.head.sha === "string" ? pull.payload.pull_request.head.sha : null; return <a key={pull.id} href={pull.url} target="_blank" rel="noreferrer" className="rounded-md border border-border/60 px-3 py-2 text-xs hover:bg-muted/30"><div className="flex items-center justify-between gap-3"><span className="font-mono text-muted-foreground">{pull.repository}#{pull.pull_request_number}</span><span className="capitalize text-muted-foreground">{pull.state}</span></div><p className="mt-1 truncate">{pull.title}</p><div className="mt-2 flex flex-wrap gap-2 text-[11px] text-muted-foreground"><span>Review: {pull.review_state ?? "unknown"}</span><span>CI: {pull.ci_state ?? "unknown"}</span>{headSha ? <span>Commit: {headSha.slice(0, 8)}</span> : null}</div></a>; })}</div>{githubPullRequestsQuery.data.truncated ? <p className="text-xs text-muted-foreground">Showing the first 100 pull requests. Refresh or open GitHub to inspect the complete list.</p> : null}</section> : null}
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
      <AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {project.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              This permanently deletes the project, its issues, documents, and agent history. Type the exact team and project names to continue.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="grid gap-3">
            <label className="grid gap-1.5 text-xs font-medium">Team name<Input value={deleteTeamName} onChange={(event) => setDeleteTeamName(event.target.value)} placeholder={team.name} autoComplete="off" /></label>
            <label className="grid gap-1.5 text-xs font-medium">Project name<Input value={deleteProjectName} onChange={(event) => setDeleteProjectName(event.target.value)} placeholder={project.name} autoComplete="off" /></label>
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleteMutation.isPending}>Cancel</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={deleteMutation.isPending || deleteTeamName !== team.name || deleteProjectName !== project.name}
              onClick={() => deleteMutation.mutate()}
            >
              {deleteMutation.isPending ? "Deleting…" : "Delete project"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </ProjectShell>
  );
}
