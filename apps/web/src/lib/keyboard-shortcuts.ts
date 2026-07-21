export type ShortcutResult = {
  buffer: string[];
  matched?: string;
};

export function advanceShortcut(buffer: string[], key: string, sequences: string[][]): ShortcutResult {
  const next = [...buffer, key.toLowerCase()];
  const exact = sequences.find((sequence) => sequence.length === next.length && sequence.every((part, index) => part === next[index]));
  if (exact) return { buffer: [], matched: exact.join(" ") };
  const isPrefix = sequences.some((sequence) => next.every((part, index) => sequence[index] === part));
  return { buffer: isPrefix ? next : [] };
}

