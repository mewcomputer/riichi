import { useEditor, EditorContent } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { useState } from "react";
import { Bold, Italic, List, Send } from "lucide-react";

import { Button } from "@/components/ui/button";

export function RichTextCommentEditor({
  submitting,
  onSubmit,
}: {
  submitting?: boolean;
  onSubmit: (content: Record<string, unknown>) => void;
}) {
  const [hasContent, setHasContent] = useState(false);
  const editor = useEditor({
    extensions: [StarterKit],
    content: { type: "doc", content: [{ type: "paragraph" }] },
    immediatelyRender: false,
    editorProps: { attributes: { "aria-label": "Comment" } },
    onUpdate: ({ editor: updatedEditor }) => setHasContent(Boolean(updatedEditor.getText().trim())),
  });

  if (!editor) return null;

  const submit = () => {
    if (!editor.getText().trim()) return;
    onSubmit(editor.getJSON() as Record<string, unknown>);
    editor.commands.clearContent();
  };

  return (
    <div className="grid gap-2 rounded-lg border border-border/70 bg-card/30 p-2 focus-within:border-ring/60">
      <EditorContent
        editor={editor}
        className="typeset min-h-20 max-w-none px-2 py-1 text-sm outline-none [&_.ProseMirror]:min-h-16 [&_.ProseMirror]:outline-none"
      />
      <div className="flex items-center gap-1 border-t border-border/50 pt-2">
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Bold" onClick={() => editor.chain().focus().toggleBold().run()}><Bold /></Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Italic" onClick={() => editor.chain().focus().toggleItalic().run()}><Italic /></Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Bullet list" onClick={() => editor.chain().focus().toggleBulletList().run()}><List /></Button>
        <Button type="button" size="sm" className="ml-auto gap-1.5" onClick={submit} disabled={submitting || !hasContent}><Send /> Comment</Button>
      </div>
    </div>
  );
}

export function RichTextComment({ content, fallback }: { content: Record<string, unknown> | null; fallback: string }) {
  const editor = useEditor({
    extensions: [StarterKit],
    content: content ?? { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: fallback }] }] },
    editable: false,
    immediatelyRender: false,
  });

  if (!editor) return <p className="whitespace-pre-wrap">{fallback}</p>;
  return <EditorContent editor={editor} className="typeset max-w-none [&_.ProseMirror]:outline-none" />;
}
