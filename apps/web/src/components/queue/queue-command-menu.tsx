import { CommandMenu } from "@/components/command/command-menu";
import type { QueueItem } from "@/data/queue";
import type { QueueAdvancedFilter, QueueFilter, QueueView } from "./types";
import { createQueueCommandGroups } from "./queue-command-groups";

export { createQueueCommandGroups } from "./queue-command-groups";

export function QueueCommandMenu({
  open,
  onOpenChange,
  onCreate,
  onFilterChange,
  onViewChange,
  onQueryChange,
  onStatusFilterChange = () => undefined,
  onImportanceFilterChange = () => undefined,
  items,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreate: () => void;
  onFilterChange: (filter: QueueFilter) => void;
  onViewChange: (view: QueueView) => void;
  onQueryChange: (query: string) => void;
  onStatusFilterChange?: (status: QueueAdvancedFilter["status"]) => void;
  onImportanceFilterChange?: (importance: QueueAdvancedFilter["importance"]) => void;
  items: QueueItem[];
}) {
  const groups = createQueueCommandGroups({ onCreate, onFilterChange, onViewChange, onQueryChange, onStatusFilterChange, onImportanceFilterChange, items });

  return <CommandMenu open={open} onOpenChange={onOpenChange} groups={groups} placeholder="Search issues or commands..." description="Search issues and actions in riichi." />;
}
