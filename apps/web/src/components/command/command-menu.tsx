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
  placeholder = "Search commands...",
  description = "Search for a command to run.",
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  groups: CommandMenuGroup[];
  placeholder?: string;
  description?: string;
}) {
  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} title="Command menu" description={description}>
      <Command className="rounded-xl border-0 bg-popover shadow-2xl">
        <CommandInput placeholder={placeholder} />
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
