import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";

export const TERMINOLOGY_HINT_STORAGE_KEY = "riichi:terminology-hint:dismissed";

export function FirstUseTerminologyHint() {
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    setVisible(window.localStorage.getItem(TERMINOLOGY_HINT_STORAGE_KEY) !== "1");
  }, []);
  if (!visible) return null;
  return (
    <section className="rounded-lg border border-primary/25 bg-primary/5 p-3 text-xs" aria-labelledby="terminology-hint-title">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h2 id="terminology-hint-title" className="font-medium">A few Riichi terms</h2>
          <dl className="mt-2 grid gap-1.5 text-muted-foreground">
            <div><dt className="inline font-medium text-foreground">Lease</dt><dd className="inline"> · time-bounded agent ownership.</dd></div>
            <div><dt className="inline font-medium text-foreground">Fencing token</dt><dd className="inline"> · the server-issued proof for a valid agent write.</dd></div>
            <div><dt className="inline font-medium text-foreground">Approval</dt><dd className="inline"> · a versioned request that needs a human decision.</dd></div>
          </dl>
        </div>
        <Button variant="ghost" size="sm" className="h-7 shrink-0" onClick={() => { window.localStorage.setItem(TERMINOLOGY_HINT_STORAGE_KEY, "1"); setVisible(false); }}>Got it</Button>
      </div>
    </section>
  );
}
