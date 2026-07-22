import { useEffect, useRef } from "react";
import { useEditor, EditorContent } from "@tiptap/react";
import { BubbleMenu } from "@tiptap/react/menus";
import StarterKit from "@tiptap/starter-kit";
import Placeholder from "@tiptap/extension-placeholder";
import { Bold, Italic, List } from "@/lib/product-icons";

import { Button } from "@/components/ui/button";

export function shouldSyncControlledEditor(
  value: string,
  editorValue: string,
  lastExternalValue: string,
) {
  return value !== lastExternalValue && value !== editorValue;
}

/**
 * Controlled Tiptap editor for the issue title.
 *
 * Plain text only — no formatting, no line breaks. The editor is constrained
 * to a single paragraph via `ensureTrailingParagraph` off and Enter key
 * suppression so the title stays on one line.
 */
export function RichTextTitleEditor({
  value,
  onChange,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  className?: string;
}) {
  const lastExternalValue = useRef(value);
  const editor = useEditor({
    extensions: [
      StarterKit,
      Placeholder.configure({ placeholder: "Issue title" }),
    ],
    content: value,
    immediatelyRender: false,
    editorProps: {
      attributes: {
        "aria-label": "Issue title",
        class: "outline-none",
      },
      handleKeyDown: (_view, event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          return true;
        }
        return false;
      },
    },
    onUpdate: ({ editor: updated }) => {
      onChange(updated.getText());
    },
  });

  // Sync external value → editor when the issue changes (e.g. navigating to a different issue)
  useEffect(() => {
    if (editor && shouldSyncControlledEditor(value, editor.getText(), lastExternalValue.current)) {
      editor.commands.setContent(value, { emitUpdate: false });
    }
    lastExternalValue.current = value;
  }, [value, editor]);

  if (!editor) return null;

  return (
    <EditorContent
      editor={editor}
      className={className ?? "text-2xl font-medium [&_.ProseMirror]:outline-none"}
    />
  );
}

/**
 * Controlled Tiptap editor for the issue body/description.
 *
 * Rich text with a floating BubbleMenu toolbar (bold, italic, bullet list)
 * that appears on text selection.
 */
export function RichTextBodyEditor({
  value,
  onChange,
  editable = true,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  editable?: boolean;
  className?: string;
}) {
  const lastExternalValue = useRef(value);
  const editor = useEditor({
    extensions: [
      StarterKit,
      Placeholder.configure({ placeholder: "Add context..." }),
    ],
    content: value || "<p></p>",
    editable,
    immediatelyRender: false,
    editorProps: {
      attributes: {
        "aria-label": "Issue description",
        class: "outline-none min-h-48",
      },
    },
    onUpdate: ({ editor: updated }) => {
      onChange(updated.getHTML());
    },
  });

  // Sync external value → editor when the issue changes
  useEffect(() => {
    if (editor && shouldSyncControlledEditor(value, editor.getHTML(), lastExternalValue.current)) {
      editor.commands.setContent(value || "<p></p>", { emitUpdate: false });
    }
    lastExternalValue.current = value;
  }, [value, editor]);

  if (!editor) return null;

  return (
    <>
      <EditorContent
        editor={editor}
        className={className ?? "typeset min-h-48 max-w-none resize-y text-sm leading-7 [&_.ProseMirror]:min-h-48 [&_.ProseMirror]:outline-none"}
      />
      <BubbleMenu editor={editor} className="flex items-center gap-0.5 rounded-lg border border-border bg-popover p-1 shadow-md">
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Bold"
          onClick={() => editor.chain().focus().toggleBold().run()}
          className={editor.isActive("bold") ? "bg-accent" : ""}
        >
          <Bold />
        </Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Italic"
          onClick={() => editor.chain().focus().toggleItalic().run()}
          className={editor.isActive("italic") ? "bg-accent" : ""}
        >
          <Italic />
        </Button>
        <Button type="button" variant="ghost" size="icon-sm" aria-label="Bullet list"
          onClick={() => editor.chain().focus().toggleBulletList().run()}
          className={editor.isActive("bulletList") ? "bg-accent" : ""}
        >
          <List />
        </Button>
      </BubbleMenu>
    </>
  );
}
