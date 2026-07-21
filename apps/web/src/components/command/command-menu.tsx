import type { LucideIcon } from "lucide-react";

import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "@/components/ui/command";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Kbd } from "@/components/ui/kbd";

export type CommandMenuItem = {
  id: string;
  label: string;
  icon: LucideIcon;
  shortcut?: string;
  keywords?: string[];
  onSelect: () => void;
};

export type CommandMenuGroup = {
  id: string;
  label: string;
  items: CommandMenuItem[];
};

export function CommandMenu({
  open,
  onOpenChange,
  groups,
  onSearchChange,
  placeholder = "Search commands...",
  description = "Search for a command to run.",
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  groups: CommandMenuGroup[];
  onSearchChange?: (query: string) => void;
  placeholder?: string;
  description?: string;
}) {
  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} title="Command menu" description={description}>
      <Command className="rounded-xl border-0 bg-popover shadow-2xl">
        <CommandInput placeholder={placeholder} onValueChange={onSearchChange} />
        <CommandList>
          <CommandEmpty>No results found.</CommandEmpty>
          {groups.map((group) => (
            <CommandGroup key={group.id} heading={group.label}>
              {group.items.map((item) => {
                const Icon = item.icon;
                return (
                  <CommandItem
                    key={item.id}
                    value={[item.label, ...(item.keywords ?? [])].join(" ")}
                    onSelect={() => {
                      item.onSelect();
                      onOpenChange(false);
                    }}
                  >
                    <Icon />
                    <span>{item.label}</span>
                    {item.shortcut ? <CommandShortcut>{item.shortcut}</CommandShortcut> : null}
                  </CommandItem>
                );
              })}
            </CommandGroup>
          ))}
        </CommandList>
      </Command>
    </CommandDialog>
  );
}

export function ShortcutReferenceDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const shortcuts = [
    ["⌘ K / Ctrl K", "Open command menu"],
    ["G I", "Open all issues"],
    ["G B", "Open backlog"],
    ["G M", "Open my work"],
    ["F R", "Show ready issues"],
    ["C", "Create an issue"],
    ["J / K", "Move through queue issues"],
    ["Enter", "Open the selected issue"],
    ["Escape", "Clear queue selection"],
  ];
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Keyboard shortcuts</DialogTitle>
          <DialogDescription>Shortcuts are also available through the command menu.</DialogDescription>
        </DialogHeader>
        <div className="grid gap-2">
          {shortcuts.map(([keys, label]) => <div key={keys} className="flex items-center justify-between gap-4 text-sm"><span>{label}</span><Kbd>{keys}</Kbd></div>)}
        </div>
      </DialogContent>
    </Dialog>
  );
}
