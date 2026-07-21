import { ChevronDown } from "lucide-react";
import { useId, useState } from "react";

import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuLabel, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import type { HumanQueueIssue } from "@/lib/api";

export type IssueImportance = HumanQueueIssue["importance"];

const importanceOptions: Array<{ value: IssueImportance; label: string }> = [
  { value: "none", label: "No priority" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
  { value: "urgent", label: "Urgent" },
];

export function issueImportanceLabel(importance: IssueImportance) {
  return importanceOptions.find((option) => option.value === importance)?.label ?? importance;
}

export function IssueImportanceIcon({ importance }: { importance: IssueImportance }) {
  const filledBars = { none: 0, low: 1, medium: 2, high: 3, urgent: 3 }[importance];
  const color = importance === "urgent" ? "text-red-400" : importance === "high" ? "text-orange-400" : "text-muted-foreground";
  const maskId = useId().replaceAll(":", "");

  if (importance === "urgent") {
    return (
      <svg aria-hidden="true" className={`size-3.5 ${color}`} viewBox="0 0 16 16" fill="none">
        <defs>
          <mask id={maskId}>
            <rect width="16" height="16" fill="white" />
            <path d="M8 4v5.5" stroke="black" strokeWidth="2" strokeLinecap="round" />
            <circle cx="8" cy="12" r="1" fill="black" />
          </mask>
        </defs>
        <circle cx="8" cy="8" r="7" fill="currentColor" mask={`url(#${maskId})`} />
      </svg>
    );
  }

  return (
    <svg aria-hidden="true" className={`size-3.5 ${color}`} viewBox="0 0 16 16" fill="none">
      {[4, 8, 12].map((height, index) => (
        <rect key={height} x={1 + index * 5} y={14 - height} width="3" height={height} rx="1" fill="currentColor" opacity={index < filledBars ? 1 : 0.2} />
      ))}
    </svg>
  );
}

export function IssueImportanceMenu({ importance, onChange, disabled = false, compact = false, className }: { importance: IssueImportance; onChange: (importance: IssueImportance) => void; disabled?: boolean; compact?: boolean; className?: string }) {
  const [open, setOpen] = useState(false);

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger render={<Button variant="ghost" size={compact ? "icon-sm" : "sm"} className={`${compact ? "text-muted-foreground" : "gap-1.5 px-2 text-xs text-muted-foreground"} ${className ?? ""}`} disabled={disabled} aria-label={compact ? `Change importance, currently ${issueImportanceLabel(importance)}` : undefined} />}>
        <IssueImportanceIcon importance={importance} /> {compact ? null : <>{issueImportanceLabel(importance)} <ChevronDown /></>}
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-44">
        <DropdownMenuLabel>Importance</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuRadioGroup value={importance} onValueChange={(value) => { if (value !== importance) { setOpen(false); onChange(value as IssueImportance); } }}>
          {importanceOptions.map((option) => <DropdownMenuRadioItem key={option.value} value={option.value}>{option.label}</DropdownMenuRadioItem>)}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
