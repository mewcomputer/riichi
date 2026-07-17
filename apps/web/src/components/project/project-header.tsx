import { Bell, Command, MoreHorizontal, type LucideIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { ReactNode } from "react";

export type ProjectViewTab = {
  value: string;
  label: string;
  icon: LucideIcon;
  count?: number;
};

export function ProjectHeader({
  view,
  views,
  onViewChange,
  onCommand,
  showNotifications = true,
  content,
  actions,
}: {
  view: string;
  views: ProjectViewTab[];
  onViewChange: (view: string) => void;
  onCommand?: () => void;
  showNotifications?: boolean;
  content?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <header className="flex h-12 shrink-0 items-center justify-between border-b border-border/70 px-3">
      <div className="flex min-w-0 items-center gap-2">
        <SidebarTrigger className="text-muted-foreground" />
        <Separator orientation="vertical" className="h-5" />
        {content ?? <>
          <Tabs
            value={view}
            onValueChange={(value) => value && onViewChange(value)}
            className="min-w-0"
          >
            <TabsList variant="line" className="h-8 gap-0 overflow-x-auto">
              {views.map(({ value, label, icon: Icon, count }) => (
                <TabsTrigger
                  key={value}
                  value={value}
                  className="h-7 shrink-0 gap-1.5 px-2.5 text-xs"
                >
                  <Icon />
                  {label}
                  {count === undefined ? null : (
                    <span className="text-[10px] text-muted-foreground">
                      {count}
                    </span>
                  )}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
          <Button variant="ghost" size="icon-sm" aria-label="More views">
            <MoreHorizontal />
          </Button>
        </>}
      </div>
      <div className="flex items-center gap-1">
        {actions}
        {showNotifications ? (
          <Button
            variant="ghost"
            size="icon-sm"
            className="text-muted-foreground"
            aria-label="Notifications"
          >
            <Bell />
          </Button>
        ) : null}
        <Button
          variant="ghost"
          size="icon-sm"
          className="text-muted-foreground"
          aria-label="Command menu"
          onClick={() => {
            if (onCommand) {
              onCommand();
            } else {
              window.dispatchEvent(new Event("riichi:open-command-menu"));
            }
          }}
        >
          <Command />
        </Button>
      </div>
    </header>
  );
}
