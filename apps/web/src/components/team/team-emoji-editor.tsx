import { useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { getProductIcon, productIconNames } from "@/lib/product-icons";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { TeamMark } from "@/components/team/team-mark";
import { updateTeamEmoji, type NavigationResponse } from "@/lib/api";

const EMOJI = ["◈", "✨", "🚀", "🧭", "🛠️", "🔭", "🌱", "🧩", "🎯", "🪴", "🦊", "🐙", "🌙", "☀️", "⚡", "💬", "📚", "🎨", "🔒", "🧪"];

export function TeamEmojiEditor({ team, canManage, onSaved }: { team: NavigationResponse["organizations"][number]["teams"][number]; canManage: boolean; onSaved: () => void }) {
  const [value, setValue] = useState(team.emoji ?? "");
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const mutation = useMutation({
    mutationFn: (next: string | null) => updateTeamEmoji(team.id, next),
    onSuccess: (_, next) => { setValue(next ?? ""); setOpen(false); onSaved(); },
  });
  const filteredIcons = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return productIconNames.filter((name) => !normalized || name.toLowerCase().includes(normalized)).slice(0, 72);
  }, [query]);
  if (!canManage) return <TeamMark value={team.emoji} className="text-lg" />;
  const save = (next: string | null) => { setValue(next ?? ""); mutation.mutate(next); };
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger render={<Button variant="outline" size="sm" className="gap-2" aria-label={`Change ${team.name} icon`} disabled={mutation.isPending} />}>
        <TeamMark value={value} className="size-4" />
        <span className="hidden sm:inline">Change mark</span>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[min(22rem,calc(100vw-2rem))]">
        <div className="grid gap-2">
          <p className="text-xs font-medium">Team mark</p>
          <p className="text-[11px] text-muted-foreground">Pick an emoji or search the full Tabler icon set.</p>
          <div className="flex flex-wrap gap-1">
            {EMOJI.map((emoji) => <button key={emoji} type="button" className={`grid size-9 place-items-center rounded-md text-lg hover:bg-accent ${value === emoji ? "bg-accent" : ""}`} onClick={() => save(emoji)} aria-label={`Use ${emoji}`}>{emoji}</button>)}
            <button type="button" className="rounded-md px-2 text-xs text-muted-foreground hover:bg-accent" onClick={() => save(null)}>Clear</button>
          </div>
          <Input aria-label="Search icons" placeholder="Search icons" value={query} onChange={(event) => setQuery(event.target.value)} />
          <div className="grid max-h-48 grid-cols-8 gap-1 overflow-y-auto" role="listbox" aria-label="Tabler icons">
            {filteredIcons.map((name) => { const Icon = getProductIcon(name); return <button key={name} type="button" className={`grid size-8 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground ${value === `tabler:${name}` ? "bg-accent text-foreground" : ""}`} onClick={() => save(`tabler:${name}`)} aria-label={`Use ${name} icon`} title={name}>{Icon ? <Icon className="size-4" aria-hidden="true" /> : null}</button>; })}
          </div>
          {filteredIcons.length === 0 ? <p className="text-xs text-muted-foreground">No icons match that search.</p> : null}
        </div>
      </PopoverContent>
    </Popover>
  );
}
