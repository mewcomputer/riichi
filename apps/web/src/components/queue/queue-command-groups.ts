import { Archive, CircleAlert, CircleCheck, Flag, Layers3, Plus, UserRound } from "lucide-react";

import type { CommandMenuGroup } from "@/components/command/command-menu";
import type { QueueItem } from "@/data/queue";
import type { QueueAdvancedFilter, QueueFilter, QueueView } from "./types";

export function createQueueCommandGroups({
  onCreate,
  onFilterChange,
  onViewChange,
  onQueryChange,
  onStatusFilterChange,
  onImportanceFilterChange,
  items,
}: {
  onCreate: () => void;
  onFilterChange: (filter: QueueFilter) => void;
  onViewChange: (view: QueueView) => void;
  onQueryChange: (query: string) => void;
  onStatusFilterChange: (status: QueueAdvancedFilter["status"]) => void;
  onImportanceFilterChange: (importance: QueueAdvancedFilter["importance"]) => void;
  items: QueueItem[];
}) {
  return [
    {
      id: "queue-actions",
      label: "Actions",
      items: [
        { id: "create-issue", label: "New issue", icon: Plus, shortcut: "C", onSelect: onCreate },
        { id: "show-ready", label: "Show ready issues", icon: CircleCheck, shortcut: "F R", onSelect: () => onFilterChange("ready") },
        { id: "open-backlog", label: "Open backlog", icon: Archive, shortcut: "G B", onSelect: () => onViewChange("backlog") },
        { id: "open-my-work", label: "Open my work", icon: UserRound, shortcut: "G M", onSelect: () => onViewChange("my_work") },
        { id: "filter-in-progress", label: "Filter in-progress issues", icon: CircleCheck, onSelect: () => onStatusFilterChange("in_progress") },
        { id: "filter-blocked", label: "Filter blocked issues", icon: CircleAlert, onSelect: () => onStatusFilterChange("blocked") },
        { id: "filter-urgent", label: "Filter urgent issues", icon: Flag, onSelect: () => onImportanceFilterChange("urgent") },
      ],
    },
    {
      id: "queue-issues",
      label: "Issues",
      items: items.slice(0, 5).map((item) => ({
        id: `issue-${item.id}`,
        label: `${item.id} · ${item.title}`,
        icon: Layers3,
        keywords: [item.id, item.title],
        onSelect: () => onQueryChange(item.id),
      })),
    },
  ] satisfies CommandMenuGroup[];
}
