import { Archive, CircleCheck, Layers3, Plus } from "lucide-react";

import type { CommandMenuGroup } from "@/components/command/command-menu";
import type { QueueItem } from "@/data/queue";
import type { QueueFilter, QueueView } from "./types";

export function createQueueCommandGroups({
  onCreate,
  onFilterChange,
  onViewChange,
  onQueryChange,
  items,
}: {
  onCreate: () => void;
  onFilterChange: (filter: QueueFilter) => void;
  onViewChange: (view: QueueView) => void;
  onQueryChange: (query: string) => void;
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
