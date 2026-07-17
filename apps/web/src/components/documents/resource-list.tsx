import { forwardRef, useEffect, useImperativeHandle, useState } from "react";

export type ResourceLinkItem = {
  id: string;
  label: string;
  description?: string;
  kind: "issue" | "team" | "project" | "document";
  href: string;
};

export type ResourceListRef = {
  onKeyDown: (props: { event: KeyboardEvent }) => boolean;
};

export const ResourceList = forwardRef<ResourceListRef, { items: ResourceLinkItem[]; command: (item: ResourceLinkItem) => void }>((props, ref) => {
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => setSelectedIndex(0), [props.items]);

  const selectItem = (index: number) => {
    const item = props.items[index];
    if (item) props.command(item);
  };

  useImperativeHandle(ref, () => ({
    onKeyDown: ({ event }) => {
      if (props.items.length === 0) return false;
      if (event.key === "ArrowUp") {
        setSelectedIndex((current) => (current + props.items.length - 1) % props.items.length);
        return true;
      }
      if (event.key === "ArrowDown") {
        setSelectedIndex((current) => (current + 1) % props.items.length);
        return true;
      }
      if (event.key === "Enter") {
        selectItem(selectedIndex);
        return true;
      }
      return false;
    },
  }), [props.items, selectedIndex]);

  return (
    <div className="w-80 overflow-hidden rounded-lg border border-border/80 bg-popover p-1 text-popover-foreground shadow-xl" role="listbox" aria-label="Riichi resources">
      <div className="px-2.5 py-2 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground">Link a resource</div>
      {props.items.length === 0 ? (
        <p className="px-3 py-2 text-xs text-muted-foreground">No resources found</p>
      ) : props.items.map((item, index) => (
        <button
          key={`${item.kind}-${item.id}`}
          type="button"
          role="option"
          aria-selected={index === selectedIndex}
          className={`flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-sm ${index === selectedIndex ? "bg-accent text-accent-foreground" : "hover:bg-accent/60"}`}
          onMouseDown={(event) => event.preventDefault()}
          onMouseEnter={() => setSelectedIndex(index)}
          onClick={() => selectItem(index)}
        >
          <span className="grid size-6 shrink-0 place-items-center rounded-full bg-muted text-[10px] font-medium uppercase">{item.kind.slice(0, 1)}</span>
          <span className="min-w-0">
            <span className="block truncate">{item.label}</span>
            {item.description ? <span className="block truncate text-xs text-muted-foreground">{item.description}</span> : null}
          </span>
        </button>
      ))}
    </div>
  );
});

ResourceList.displayName = "ResourceList";
