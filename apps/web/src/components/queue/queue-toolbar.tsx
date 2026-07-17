import { Plus, RefreshCw, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export function QueueToolbar({
  query,
  onQueryChange,
  refreshing,
  onRefresh,
  onCreate,
}: {
  query: string;
  onQueryChange: (query: string) => void;
  refreshing: boolean;
  onRefresh: () => void;
  onCreate: () => void;
}) {
  return (
    <>
      <div className="flex h-11 shrink-0 items-center justify-between border-b border-border/60 px-4">
        <div className="flex min-w-0 items-center gap-2">
          <div className="relative w-48">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(event) => onQueryChange(event.target.value)}
              placeholder="Search issues"
              className="h-7 border-transparent bg-transparent pl-8 text-xs shadow-none focus-visible:bg-muted/50"
            />
          </div>
        </div>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon-sm" className="text-muted-foreground" aria-label="Refresh queue" onClick={onRefresh} disabled={refreshing}>
            <RefreshCw className={refreshing ? "animate-spin" : ""} />
          </Button>
          <Button size="sm" className="h-7 gap-1.5 px-2.5 text-xs" onClick={onCreate}>
            <Plus /> New issue
          </Button>
        </div>
      </div>
    </>
  );
}
