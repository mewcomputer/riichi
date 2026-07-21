import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { createProject, getCurrentUser } from "@/lib/api";
import { useNavigation } from "../hooks/use-navigation";
import { organizationSlug as toOrganizationSlug } from "../lib/organization-slug";

export function OnboardingPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const meQuery = useQuery({ queryKey: ["auth", "me"], queryFn: getCurrentUser, retry: false });
  const navigationQuery = useNavigation();
  const [projectName, setProjectName] = useState("");
  const projectMutation = useMutation({
    mutationFn: () => createProject(projectName.trim()),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["auth", "me"] });
      void queryClient.invalidateQueries({ queryKey: ["navigation"] });
      void navigate({ to: "/$organizationSlug/issues", params: { organizationSlug: toOrganizationSlug(navigationQuery.data?.organizations[0]?.name ?? "Riichi") }, search: { guide: "1" } as never, replace: true });
    },
  });

  return (
    <main className="grid min-h-svh place-items-center bg-background px-6 text-foreground">
      <section className="w-full max-w-md">
        <div className="mb-8 flex items-center gap-3">
          <span className="grid size-8 place-items-center rounded-lg bg-foreground text-sm font-semibold text-background">R</span>
          <span className="text-sm font-medium">Riichi</span>
        </div>
        <div className="grid gap-2">
          <p className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">First-time setup</p>
          <h1 className="text-3xl font-medium tracking-tight">Create your first project</h1>
          <p className="text-sm leading-6 text-muted-foreground">Projects hold your issues and agent work. You can invite collaborators after setup.</p>
        </div>
        <div className="mt-8 grid gap-3">
          <label className="grid gap-1.5 text-xs font-medium" htmlFor="project-name">Project name</label>
          <div className="flex gap-2">
            <Input id="project-name" autoFocus value={projectName} onChange={(event) => setProjectName(event.target.value)} placeholder="e.g. Northstar" />
            <Button onClick={() => projectMutation.mutate()} disabled={projectMutation.isPending || !projectName.trim()}>Create</Button>
          </div>
          {projectMutation.error ? <p className="text-xs text-destructive">{projectMutation.error.message}</p> : null}
        </div>
        {meQuery.error ? <p className="mt-6 text-xs text-destructive">Your session could not be loaded. Please sign in again.</p> : null}
      </section>
    </main>
  );
}
