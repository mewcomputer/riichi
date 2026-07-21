import { describe, expect, it, vi } from "vitest";

import { createQueueCommandGroups } from "./queue-command-groups";

describe("queue command groups", () => {
  it("exposes visible status and priority filters through the command menu", () => {
    const onStatusFilterChange = vi.fn();
    const onImportanceFilterChange = vi.fn();
    const [actions] = createQueueCommandGroups({
      onCreate: vi.fn(),
      onFilterChange: vi.fn(),
      onViewChange: vi.fn(),
      onQueryChange: vi.fn(),
      onStatusFilterChange,
      onImportanceFilterChange,
      items: [],
    });
    const items = actions.items as Array<{ id: string; onSelect: () => void }>;
    items.find((item) => item.id === "filter-blocked")?.onSelect();
    items.find((item) => item.id === "filter-urgent")?.onSelect();
    expect(onStatusFilterChange).toHaveBeenCalledWith("blocked");
    expect(onImportanceFilterChange).toHaveBeenCalledWith("urgent");
  });
});
