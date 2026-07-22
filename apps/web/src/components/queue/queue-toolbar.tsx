import { Plus, RefreshCw, Search } from "@/lib/product-icons";

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
      <div className="flex min-h-11 shrink-0 items-center justify-between gap-2 border-b border-border/60 px-3 sm:px-4">
        <div className="flex min-w-0 flex-1 items-center gap-2">
          <div className="relative w-full sm:w-48">
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
          <Button variant="ghost" size="icon-sm" className="size-11 text-muted-foreground sm:size-7" aria-label="Refresh queue" onClick={onRefresh} disabled={refreshing}>
            <RefreshCw className={refreshing ? "animate-spin" : ""} />
          </Button>
          <Button size="sm" className="h-11 gap-1.5 px-2 sm:h-7 sm:px-2.5" onClick={onCreate} aria-label="New issue">
            <Plus /><span className="hidden sm:inline">New issue</span>
          </Button>
        </div>
      </div>
    </>
  );
}
