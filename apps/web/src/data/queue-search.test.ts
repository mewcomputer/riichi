import { describe, expect, it } from "vitest";

import { activeQueueFilterChips, moveQueueSelection, parseQueueSearch, serializeQueueSearch } from "./queue-search";

describe("queue search state", () => {
  it("parses valid URL state and preserves the queue controls", () => {
    expect(parseQueueSearch({
      filter: "ready",
      view: "backlog",
      q: "ENG-42",
      details: "0",
      status: "blocked",
      importance: "urgent",
      team: "eng",
      project: "project-1",
    })).toEqual({
      filter: "ready",
      view: "backlog",
      query: "ENG-42",
      showDetails: false,
      advancedFilter: { status: "blocked", importance: "urgent", teamKey: "eng", projectId: "project-1", assignee: "all", label: "all" },
    });
  });

  it("falls back safely for malformed or missing URL values", () => {
    expect(parseQueueSearch({ filter: "cycles", view: "week", status: "unknown", importance: 3, q: 42, team: "" })).toEqual({
      filter: "all",
      view: "all",
      query: "",
      showDetails: true,
      advancedFilter: { status: "all", importance: "all", teamKey: "all", projectId: "all", assignee: "all", label: "all" },
    });
  });

  it("preserves the URL-backed My work preset", () => {
    const state = parseQueueSearch({ view: "my_work" });
    expect(state.view).toBe("my_work");
    expect(serializeQueueSearch(state)).toEqual({ view: "my_work" });
  });

  it("omits defaults so copied URLs stay short", () => {
    expect(serializeQueueSearch({
      filter: "all",
      view: "all",
      query: "",
      showDetails: true,
      advancedFilter: { status: "all", importance: "all", teamKey: "all", projectId: "all", assignee: "all", label: "all" },
    })).toEqual({});
  });

  it("moves from the edges without wrapping around", () => {
    const ids = ["one", "two", "three"];
    expect(moveQueueSelection(ids, null, 1)).toBe("one");
    expect(moveQueueSelection(ids, null, -1)).toBe("three");
    expect(moveQueueSelection(ids, "one", -1)).toBe("one");
    expect(moveQueueSelection(ids, "three", 1)).toBe("three");
    expect(moveQueueSelection(ids, "two", 1)).toBe("three");
    expect(moveQueueSelection([], null, 1)).toBeNull();
  });

  it("describes active filters with independent clear operations", () => {
    const chips = activeQueueFilterChips({
      filter: "ready",
      view: "backlog",
      query: "ENG-42",
      showDetails: true,
      advancedFilter: { status: "blocked", importance: "urgent", teamKey: "eng", projectId: "project-1", assignee: "me", label: "urgent" },
    }, { team: "Engineering", project: "Dispatch" });
    expect(chips.map((chip) => chip.label)).toEqual([
      "Ready",
      "Backlog",
      "Search: ENG-42",
      "Status: Blocked",
      "Priority: Urgent",
      "Team: Engineering",
      "Project: Dispatch",
      "Assignee: Me",
      "Label: urgent",
    ]);
    expect(chips.find((chip) => chip.id === "status")?.clear).toEqual({
      advancedFilter: { status: "all", importance: "urgent", teamKey: "eng", projectId: "project-1", assignee: "me", label: "urgent" },
    });
  });
});
