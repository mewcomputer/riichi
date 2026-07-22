import { forwardRef, useEffect, useImperativeHandle, useState } from "react";
import type { ProductIcon } from "@/lib/product-icons";
import type { Editor } from "@tiptap/core";

export type SlashCommandItem = {
  id: string;
  label: string;
  description: string;
  icon: ProductIcon;
  command: (editor: Editor, range: { from: number; to: number }) => void;
};

export type SlashCommandListRef = {
  onKeyDown: (props: { event: KeyboardEvent }) => boolean;
};

export type SlashCommandListProps = {
  items: SlashCommandItem[];
  command: (item: SlashCommandItem) => void;
};

export const SlashCommandList = forwardRef<SlashCommandListRef, SlashCommandListProps>((props, ref) => {
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
    <div className="w-80 overflow-hidden rounded-lg border border-border/80 bg-popover p-1 text-popover-foreground shadow-xl" role="listbox" aria-label="Insert block">
      <div className="px-2.5 py-2 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground">Insert block</div>
      {props.items.length === 0 ? (
        <p className="px-3 py-2 text-xs text-muted-foreground">No commands found</p>
      ) : props.items.map((item, index) => {
        const Icon = item.icon;
        return (
          <button
            key={item.id}
            type="button"
            role="option"
            aria-selected={index === selectedIndex}
            className={`flex w-full items-center gap-2.5 rounded-md px-2.5 py-2 text-left ${index === selectedIndex ? "bg-accent text-accent-foreground" : "hover:bg-accent/60"}`}
            onMouseDown={(event) => event.preventDefault()}
            onMouseEnter={() => setSelectedIndex(index)}
            onClick={() => selectItem(index)}
          >
            <span className="grid size-7 shrink-0 place-items-center rounded-md border border-border/70 bg-muted/40">
              <Icon className="size-4" />
            </span>
            <span className="min-w-0">
              <span className="block text-sm">{item.label}</span>
              <span className="block truncate text-xs text-muted-foreground">{item.description}</span>
            </span>
          </button>
        );
      })}
      <div className="flex items-center gap-2 px-2.5 py-2 text-[10px] text-muted-foreground">
        <kbd className="rounded border border-border/70 px-1">↑↓</kbd>
        <span>navigate</span>
        <kbd className="ml-auto rounded border border-border/70 px-1">↵</kbd>
        <span>insert</span>
      </div>
    </div>
  );
});

SlashCommandList.displayName = "SlashCommandList";
