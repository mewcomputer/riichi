import type { MentionItem } from "./mention-list";
import type { SlashCommandItem } from "./slash-command-list";

export function filterMentionItems(items: MentionItem[], query: string) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  return items.filter((item) => `${item.label} ${item.description ?? ""}`.toLocaleLowerCase().includes(normalizedQuery)).slice(0, 8);
}

export function filterSlashCommandItems(items: SlashCommandItem[], query: string) {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  return items.filter((item) => `${item.label} ${item.description}`.toLocaleLowerCase().includes(normalizedQuery)).slice(0, 8);
}
