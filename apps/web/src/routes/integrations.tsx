import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "@tanstack/react-router";

import { ApiError, getCurrentUser, getGithubIntegration, importGithubIssues, refreshGithubPullRequests, setGithubIntegration } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ProjectHeader } from "@/components/project/project-header";
import { ProjectShell } from "@/components/project/project-shell";
import { ProjectSidebar } from "@/components/project/project-sidebar";
import { useAppLogout } from "../hooks/use-app-logout";
import { useActiveProject } from "../hooks/use-active-project";
import { useNavigation } from "../hooks/use-navigation";

export function IntegrationsPage() {
  const [repository, setRepository] = useState("");
  const [maxIssues, setMaxIssues] = useState("100");
  const [maxPullRequests, setMaxPullRequests] = useState("50");
  const navigate = useNavigate();
  const { organizationSlug } = useParams({ from: "/$organizationSlug/integrations" });
  const appLogout = useAppLogout();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const { activeMembership, projectId, selectProject } = useActiveProject(meQuery.data?.memberships);
  const projectName = activeMembership?.project_name ?? "riichi";
  const integrationQuery = useQuery({ queryKey: ["github-integration", projectId], queryFn: () => getGithubIntegration(projectId!), enabled: Boolean(projectId) });
  useEffect(() => { if (integrationQuery.data?.repository && !repository) setRepository(integrationQuery.data.repository); }, [integrationQuery.data, repository]);
  const importMutation = useMutation({
    mutationFn: () => importGithubIssues(projectId!, {
      repository: repository.trim(),
      max_issues: Number(maxIssues),
    }),
  });
  const pullRequestMutation = useMutation({
    mutationFn: () => refreshGithubPullRequests(projectId!, { repository: repository.trim(), max_pull_requests: Number(maxPullRequests) }),
  });
  const integrationMutation = useMutation({
    mutationFn: () => setGithubIntegration(projectId!, { repository: repository.trim(), enabled: true }),
    onSuccess: () => void integrationQuery.refetch(),
  });
  const error = meQuery.error ?? integrationMutation.error ?? importMutation.error ?? pullRequestMutation.error;

  return (
    <ProjectShell sidebar={<ProjectSidebar projectName={projectName} navigation={navigationQuery.data} memberships={meQuery.data?.memberships} activeProjectId={projectId} onProjectChange={selectProject} onLogout={appLogout} avatarUrl={meQuery.data?.avatar_url} onSearch={() => undefined} onNavigate={(label) => {
      if (label === "Issues") void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug } });
      if (label === "Agents") void navigate({ to: "/$organizationSlug/agents", params: { organizationSlug } });
      if (label === "Invite people") void navigate({ to: "/$organizationSlug/settings", params: { organizationSlug } });
    }} userName={meQuery.data?.display_name ?? "Alex Morgan"} />}>
      <ProjectHeader view="all" views={[]} onViewChange={() => undefined} />
      <main className="mx-auto flex w-full max-w-3xl flex-col gap-6 px-8 py-8">
        <div>
          <Link to="/$organizationSlug/issues" params={{ organizationSlug }} className="text-xs text-muted-foreground hover:text-foreground">← Queue</Link>
          <h1 className="mt-3 text-2xl font-medium tracking-tight">GitHub integration</h1>
          <p className="mt-1 text-sm text-muted-foreground">Import issue snapshots into this project. Pull requests are filtered out and external text stays untrusted.</p>
        </div>
        <section className="grid gap-4 border-y border-border/60 py-5">
          <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_7rem_7rem] sm:items-end">
            <label className="grid min-w-0 gap-1.5 text-xs text-muted-foreground">Repository
              <Input aria-label="GitHub repository" value={repository} onChange={(event) => setRepository(event.target.value)} placeholder="owner/repository" className="h-9 text-xs" />
            </label>
            <label className="grid gap-1.5 text-xs text-muted-foreground">Max issues
              <Input aria-label="Maximum issues" type="number" min="1" max="1000" value={maxIssues} onChange={(event) => setMaxIssues(event.target.value)} className="h-9 text-xs" />
            </label>
            <label className="grid gap-1.5 text-xs text-muted-foreground">Max PRs
              <Input aria-label="Maximum pull requests" type="number" min="1" max="100" value={maxPullRequests} onChange={(event) => setMaxPullRequests(event.target.value)} className="h-9 text-xs" />
            </label>
          </div>
          <div className="grid gap-2 sm:flex sm:flex-wrap sm:items-center">
            <Button className="w-full sm:w-auto" size="sm" variant="outline" onClick={() => integrationMutation.mutate()} disabled={integrationMutation.isPending || !repository.trim()}>Save integration</Button>
            <Button className="w-full sm:w-auto" size="sm" onClick={() => importMutation.mutate()} disabled={importMutation.isPending || pullRequestMutation.isPending || !repository.trim()}>Import issues</Button>
            <Button className="w-full sm:w-auto" size="sm" variant="outline" onClick={() => pullRequestMutation.mutate()} disabled={importMutation.isPending || pullRequestMutation.isPending || !repository.trim()}>Refresh pull requests</Button>
          </div>
          {error ? <span className="text-xs text-destructive">{error instanceof ApiError && error.status === 401 ? "Sign in with an admin project membership." : error.message}</span> : null}
          {importMutation.data ? <div className="flex items-center gap-2 text-xs"><Badge variant="secondary">{importMutation.data.imported} imported</Badge><Badge variant="outline">{importMutation.data.pull_requests_skipped} pull requests skipped</Badge></div> : null}
          {pullRequestMutation.data ? <Badge variant="secondary">{pullRequestMutation.data.imported} pull requests refreshed</Badge> : null}
        </section>
      </main>
    </ProjectShell>
  );
}
