import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";

import { getNavigation } from "@/lib/api";

type NavigationSyncModule = typeof import("../lib/navigation-sync");
type NavigationCollection = NonNullable<ReturnType<NavigationSyncModule["createNavigationCollection"]>>;

export function useNavigation() {
  const query = useQuery({
    queryKey: ["navigation"],
    queryFn: getNavigation,
    retry: false,
  });
  const [syncModule, setSyncModule] = useState<NavigationSyncModule | null>(null);
  const [collection, setCollection] = useState<NavigationCollection | null>(null);
  const [, setRevision] = useState(0);
  useEffect(() => {
    if (import.meta.env.VITE_ELECTRIC_SYNC_ENABLED !== "true") return;
    let active = true;
    void import("../lib/navigation-sync").then((module) => {
      if (active) setSyncModule(module);
    });
    return () => {
      active = false;
    };
  }, []);
  useEffect(() => {
    if (!syncModule) return;
    const next = syncModule.createNavigationCollection();
    setCollection(next);
    if (!next) return;
    const rerender = () => setRevision((revision) => revision + 1);
    const subscription = next.subscribeChanges(rerender, {
      includeInitialState: true,
      onStatusChange: rerender,
    });
    void next.preload().then(rerender, rerender);
    return () => subscription.unsubscribe();
  }, [syncModule]);
  const replicated = collection?.isReady() && syncModule
    ? syncModule.navigationFromSyncRows(collection.toArray)
    : undefined;
  const data = replicated && query.data
    ? {
        ...replicated,
        organizations: replicated.organizations.map((organization) => {
          const serverOrganization = query.data?.organizations.find((candidate) => candidate.id === organization.id);
          return {
            ...organization,
            teams: organization.teams.map((team) => {
              const serverTeam = serverOrganization?.teams.find((candidate) => candidate.id === team.id);
              return {
                ...team,
                emoji: serverTeam ? serverTeam.emoji : team.emoji,
                projects: team.projects.map((project) => {
                  const serverProject = serverTeam?.projects.find((candidate) => candidate.id === project.id);
                  return { ...project, icon: serverProject ? serverProject.icon : project.icon };
                }),
              };
            }),
          };
        }),
      }
    : replicated ?? query.data;

  return {
    ...query,
    data,
  };
}
