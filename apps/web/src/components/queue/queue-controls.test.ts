import { describe, expect, it } from "vitest";

import { requiresBulkActionConfirmation } from "./queue-controls";

describe("queue bulk action risk", () => {
  it("requires confirmation only for bulk cancellation", () => {
    expect(requiresBulkActionConfirmation({ kind: "status", value: "canceled" })).toBe(true);
    expect(requiresBulkActionConfirmation({ kind: "status", value: "done" })).toBe(false);
    expect(requiresBulkActionConfirmation({ kind: "importance", value: "urgent" })).toBe(false);
  });
});
