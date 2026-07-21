import { describe, expect, it } from "vitest";

import { bulkResultCopy, requiresBulkActionConfirmation } from "./queue-controls";

describe("queue bulk action risk", () => {
  it("requires confirmation only for bulk cancellation", () => {
    expect(requiresBulkActionConfirmation({ kind: "status", value: "canceled" })).toBe(true);
    expect(requiresBulkActionConfirmation({ kind: "status", value: "done" })).toBe(false);
    expect(requiresBulkActionConfirmation({ kind: "importance", value: "urgent" })).toBe(false);
  });
});

describe("queue bulk result summary", () => {
  it("calls out partial rejection without hiding confirmed work", () => {
    expect(bulkResultCopy({ total: 3, confirmed: 2, rejected: 1 })).toBe("2 saved, 1 rejected");
    expect(bulkResultCopy({ total: 2, confirmed: 2, rejected: 0 })).toBe("2 saved");
  });
});
