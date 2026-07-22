import { ChevronDown } from "@/lib/product-icons";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { HumanQueueIssue } from "@/lib/api";

export type IssueStatus = HumanQueueIssue["status"];

export const issueStatuses: Array<{ value: IssueStatus; label: string }> = [
  { value: "triage", label: "Triage" },
  { value: "todo", label: "Todo" },
  { value: "in_progress", label: "In progress" },
  { value: "blocked", label: "Blocked" },
  { value: "done", label: "Done" },
  { value: "canceled", label: "Canceled" },
];

export function issueStatusLabel(status: IssueStatus) {
  return issueStatuses.find((option) => option.value === status)?.label ?? status;
}

export function IssueStatusMenu({
  status,
  onChange,
  disabled = false,
  icon,
  className,
}: {
  status: IssueStatus;
  onChange: (status: IssueStatus) => void;
  disabled?: boolean;
  icon?: React.ReactNode;
  className?: string;
}) {
  const [open, setOpen] = useState(false);

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger
        render={<Button variant={icon ? "ghost" : "outline"} size={icon ? "icon-sm" : "sm"} className={`${icon ? "text-muted-foreground hover:text-foreground" : "gap-1.5"} ${className ?? ""}`} disabled={disabled} aria-label={icon ? `Change status, currently ${issueStatusLabel(status)}` : undefined} />}
      >
        {icon ?? <>{issueStatusLabel(status)} <ChevronDown /></>}
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-44">
        <DropdownMenuLabel>Change status</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuRadioGroup value={status} onValueChange={(value) => { if (value !== status) { setOpen(false); onChange(value as IssueStatus); } }}>
          {issueStatuses.map((option) => <DropdownMenuRadioItem key={option.value} value={option.value}>{option.label}</DropdownMenuRadioItem>)}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
