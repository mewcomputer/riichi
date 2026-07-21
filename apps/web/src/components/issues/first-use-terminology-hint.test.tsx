import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";

import { FirstUseTerminologyHint, TERMINOLOGY_HINT_STORAGE_KEY } from "./first-use-terminology-hint";

describe("FirstUseTerminologyHint", () => {
  beforeEach(() => window.localStorage.clear());

  it("shows the authority terms once and remembers dismissal", () => {
    const { rerender } = render(<FirstUseTerminologyHint />);
    expect(screen.getByRole("heading", { name: "A few Riichi terms" })).toBeInTheDocument();
    expect(screen.getByText(/server-issued proof/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Got it" }));
    expect(screen.queryByRole("heading", { name: "A few Riichi terms" })).not.toBeInTheDocument();
    expect(window.localStorage.getItem(TERMINOLOGY_HINT_STORAGE_KEY)).toBe("1");
    rerender(<FirstUseTerminologyHint />);
    expect(screen.queryByRole("heading", { name: "A few Riichi terms" })).not.toBeInTheDocument();
  });
});
