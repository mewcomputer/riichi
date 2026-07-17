import { useState } from "react";
import { useMutation } from "@tanstack/react-query";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { updateTeamEmoji, type NavigationResponse } from "@/lib/api";

export function TeamEmojiEditor({
  team,
  canManage,
  onSaved,
}: {
  team: NavigationResponse["organizations"][number]["teams"][number];
  canManage: boolean;
  onSaved: () => void;
}) {
  const [emoji, setEmoji] = useState(team.emoji ?? "");
  const mutation = useMutation({
    mutationFn: () => updateTeamEmoji(team.id, emoji.trim() || null),
    onSuccess: () => {
      setEmoji(emoji.trim());
      onSaved();
    },
  });

  if (!canManage) return <span className="text-lg">{team.emoji ?? "·"}</span>;
  return (
    <div className="flex items-center gap-2">
      <Input aria-label={`${team.name} emoji`} value={emoji} onChange={(event) => setEmoji(event.target.value)} placeholder="Emoji" maxLength={8} className="h-7 w-20 text-center text-sm" />
      <Button size="sm" variant="outline" onClick={() => mutation.mutate()} disabled={mutation.isPending}>
        {mutation.isPending ? "Saving…" : "Save"}
      </Button>
    </div>
  );
}
