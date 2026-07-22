import { forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import type { ComponentType, ReactNode } from "react";
import { EditorContent, ReactRenderer, useEditor } from "@tiptap/react";
import { BubbleMenu } from "@tiptap/react/menus";
import { Extension, Node, mergeAttributes } from "@tiptap/core";
import { DragHandle } from "@tiptap/extension-drag-handle-react";
import Image from "@tiptap/extension-image";
import Link from "@tiptap/extension-link";
import Mention from "@tiptap/extension-mention";
import Placeholder from "@tiptap/extension-placeholder";
import StarterKit from "@tiptap/starter-kit";
import TaskItem from "@tiptap/extension-task-item";
import TaskList from "@tiptap/extension-task-list";
import Suggestion, { type SuggestionKeyDownProps, type SuggestionProps } from "@tiptap/suggestion";
import { PluginKey } from "@tiptap/pm/state";
import {
  Bold,
  Code2,
  Italic,
  GripVertical,
  ListTodo,
  Link as LinkIcon,
  List,
  ListOrdered,
  Minus,
  Paperclip,
  Plus,
  Quote,
  Strikethrough,
  Type,
} from "@/lib/product-icons";

import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { createLoroEditorPlugins, type LoroDocumentSession } from "@/lib/loro-document";
import { normalizeDocumentContent } from "@/lib/document-content";
import { filterMentionItems, filterSlashCommandItems } from "./editor-suggestions";
import { MentionList, type MentionItem, type MentionListRef } from "./mention-list";
import { filterResourceItems } from "./document-references";
import { ResourceList, type ResourceLinkItem, type ResourceListRef } from "./resource-list";
import { SlashCommandList, type SlashCommandItem, type SlashCommandListRef } from "./slash-command-list";

type DocumentContent = Record<string, unknown>;
type SuggestionListRef = { onKeyDown: (props: { event: KeyboardEvent }) => boolean };

const slashCommandPluginKey = new PluginKey("document-slash-command");
const resourceLinkPluginKey = new PluginKey("document-resource-link");

function createSlashCommands(schemaVersion: number, onRequestAttachment?: () => void): SlashCommandItem[] {
  const commands: SlashCommandItem[] = [
  {
    id: "paragraph",
    label: "Paragraph",
    description: "Start with plain text",
    icon: Type,
    command: (editor) => editor.chain().focus().setParagraph().run(),
  },
  {
    id: "heading-1",
    label: "Heading 1",
    description: "A large section heading",
    icon: Type,
    command: (editor) => editor.chain().focus().setHeading({ level: 1 }).run(),
  },
  {
    id: "heading-2",
    label: "Heading 2",
    description: "A medium section heading",
    icon: Type,
    command: (editor) => editor.chain().focus().setHeading({ level: 2 }).run(),
  },
  {
    id: "heading-3",
    label: "Heading 3",
    description: "A small section heading",
    icon: Type,
    command: (editor) => editor.chain().focus().setHeading({ level: 3 }).run(),
  },
  {
    id: "bullet-list",
    label: "Bullet list",
    description: "Make a simple list",
    icon: List,
    command: (editor) => editor.chain().focus().toggleBulletList().run(),
  },
  {
    id: "numbered-list",
    label: "Numbered list",
    description: "Make a sequenced list",
    icon: ListOrdered,
    command: (editor) => editor.chain().focus().toggleOrderedList().run(),
  },
  {
    id: "task-list",
    label: "Task list",
    description: "Track items with checkboxes",
    icon: ListTodo,
    command: (editor) => editor.chain().focus().toggleTaskList().run(),
  },
  {
    id: "quote",
    label: "Quote",
    description: "Set off a useful passage",
    icon: Quote,
    command: (editor) => editor.chain().focus().toggleBlockquote().run(),
  },
  {
    id: "code-block",
    label: "Code block",
    description: "Show a larger code sample",
    icon: Code2,
    command: (editor) => editor.chain().focus().toggleCodeBlock().run(),
  },
  {
    id: "divider",
    label: "Divider",
    description: "Separate related sections",
    icon: Minus,
    command: (editor) => editor.chain().focus().setHorizontalRule().run(),
  },
  ];
  if (schemaVersion >= 2) {
    commands.push({
      id: "callout",
      label: "Callout",
      description: "Highlight an important note",
      icon: Type,
      command: (editor) => editor.chain().focus().insertContent({
        type: "callout",
        attrs: { kind: "info" },
        content: [{ type: "paragraph" }],
      }).run(),
    });
  }
  if (onRequestAttachment) {
    commands.push({
      id: "attachment",
      label: "Attachment",
      description: "Upload an image",
      icon: Paperclip,
      command: () => onRequestAttachment(),
    });
  }
  return commands;
}

function createSuggestionRenderer<R extends SuggestionListRef>(Component: ComponentType<any>) {
  let renderer: ReactRenderer<R> | null = null;
  let unmount: (() => void) | null = null;

  return {
    onStart: (props: SuggestionProps<any, any>) => {
      renderer = new ReactRenderer<R>(Component, { editor: props.editor, props });
      unmount = props.mount(renderer.element);
    },
    onUpdate: (props: SuggestionProps<any, any>) => {
      renderer?.updateProps(props);
    },
    onKeyDown: (props: SuggestionKeyDownProps) => renderer?.ref?.onKeyDown(props) ?? false,
    onExit: () => {
      unmount?.();
      unmount = null;
      renderer?.destroy();
      renderer = null;
    },
  };
}

function createSlashCommandExtension(commands: SlashCommandItem[]) {
  return Extension.create({
    name: "document-slash-command",
    addProseMirrorPlugins() {
      return [
        Suggestion<SlashCommandItem, SlashCommandItem>({
          editor: this.editor,
          pluginKey: slashCommandPluginKey,
          char: "/",
          startOfLine: true,
          items: ({ query }) => filterSlashCommandItems(commands, query),
          command: ({ editor, range, props }) => {
            editor.chain().focus().deleteRange(range).run();
            props.command(editor, range);
          },
          render: () => createSuggestionRenderer<SlashCommandListRef>(SlashCommandList),
        }),
      ];
    },
  });
}

const AttachmentImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      attachmentId: { default: null },
    };
  },
});

const ResourceLink = Link.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      resourceKind: {
        default: null,
        parseHTML: (element: HTMLElement) => element.getAttribute("data-riichi-resource-kind"),
        renderHTML: (attributes: Record<string, unknown>) => attributes.resourceKind
          ? { "data-riichi-resource-kind": attributes.resourceKind }
          : {},
      },
      resourceId: {
        default: null,
        parseHTML: (element: HTMLElement) => element.getAttribute("data-riichi-resource-id"),
        renderHTML: (attributes: Record<string, unknown>) => attributes.resourceId
          ? { "data-riichi-resource-id": attributes.resourceId }
          : {},
      },
      sourceBlockId: {
        default: null,
        parseHTML: (element: HTMLElement) => element.getAttribute("data-riichi-source-block-id"),
        renderHTML: (attributes: Record<string, unknown>) => attributes.sourceBlockId
          ? { "data-riichi-source-block-id": attributes.sourceBlockId }
          : {},
      },
    };
  },
});

export const DocumentCallout = Node.create({
  name: "callout",
  group: "block",
  content: "block+",
  defining: true,
  addAttributes() {
    return {
      kind: {
        default: "info",
        parseHTML: (element: HTMLElement) => element.getAttribute("data-callout") ?? "info",
        renderHTML: (attributes: Record<string, unknown>) => ({
          "data-callout": attributes.kind ?? "info",
        }),
      },
    };
  },
  parseHTML() {
    return [{ tag: "aside[data-callout]" }];
  },
  renderHTML({ HTMLAttributes }) {
    return ["aside", mergeAttributes(HTMLAttributes), 0];
  },
});

function createResourceLinkExtension(itemsRef: { current: ResourceLinkItem[] }) {
  return Extension.create({
    name: "document-resource-link-suggestion",
    addProseMirrorPlugins() {
      return [
        Suggestion<ResourceLinkItem, ResourceLinkItem>({
          editor: this.editor,
          pluginKey: resourceLinkPluginKey,
          char: "#",
          items: ({ query }) => filterResourceItems(itemsRef.current, query),
          command: ({ editor, range, props }) => {
            editor.chain().focus().deleteRange(range).insertContent({
              type: "text",
              text: props.label,
              marks: [{
                type: "link",
                attrs: {
                  href: props.href,
                  resourceKind: props.kind,
                  resourceId: props.id,
                  sourceBlockId: crypto.randomUUID(),
                },
              }],
            }).run();
          },
          render: () => createSuggestionRenderer<ResourceListRef>(ResourceList),
        }),
      ];
    },
  });
}

function ToolbarButton({ active, label, children, onClick }: { active?: boolean; label: string; children: ReactNode; onClick: () => void }) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      aria-label={label}
      aria-pressed={active}
      className={active ? "bg-accent text-accent-foreground" : ""}
      onMouseDown={(event) => event.preventDefault()}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}

export type DocumentEditorHandle = {
  insertAttachment: (attachment: { src: string; alt: string; attachmentId: string }) => void;
};

type DocumentEditorProps = {
  value: DocumentContent;
  onChange: (value: DocumentContent) => void;
  onRequestAttachment?: () => void;
  loroSession?: LoroDocumentSession;
  schemaVersion?: number;
  mentionItems?: MentionItem[];
  resourceItems?: ResourceLinkItem[];
};

export const DocumentEditor = forwardRef<DocumentEditorHandle, DocumentEditorProps>(function DocumentEditor({
  value,
  onChange,
  onRequestAttachment,
  loroSession,
  schemaVersion = 1,
  mentionItems = [],
  resourceItems = [],
}, ref) {
  const mentionItemsRef = useRef(mentionItems);
  const resourceItemsRef = useRef(resourceItems);
  const onRequestAttachmentRef = useRef(onRequestAttachment);
  const [blockMenuOpen, setBlockMenuOpen] = useState(false);
  const [activeNodePos, setActiveNodePos] = useState<number | null>(null);
  useEffect(() => {
    mentionItemsRef.current = mentionItems;
  }, [mentionItems]);
  useEffect(() => {
    resourceItemsRef.current = resourceItems;
  }, [resourceItems]);
  useEffect(() => {
    onRequestAttachmentRef.current = onRequestAttachment;
  }, [onRequestAttachment]);
  const canRequestAttachment = Boolean(onRequestAttachment);
  const slashCommands = useMemo(
    () => createSlashCommands(schemaVersion, canRequestAttachment ? () => onRequestAttachmentRef.current?.() : undefined),
    [canRequestAttachment, schemaVersion],
  );
  const mentionExtension = useMemo(() => Mention.configure({
    HTMLAttributes: { class: "rounded bg-accent px-1 text-accent-foreground" },
    suggestion: {
      char: "@",
      items: ({ query }: { query: string }) => filterMentionItems(mentionItemsRef.current, query),
      render: () => createSuggestionRenderer<MentionListRef>(MentionList),
    },
  }), []);
  const slashExtension = useMemo(() => createSlashCommandExtension(slashCommands), [slashCommands]);
  const resourceLinkExtension = useMemo(() => createResourceLinkExtension(resourceItemsRef), []);
  const editor = useEditor({
    extensions: [
      StarterKit.configure({ link: false }),
      TaskList,
      TaskItem.configure({ nested: true }),
      ResourceLink.configure({ openOnClick: false, autolink: true, linkOnPaste: true }),
      AttachmentImage.configure({ allowBase64: false }),
      ...(schemaVersion >= 2 ? [DocumentCallout] : []),
      mentionExtension,
      slashExtension,
      resourceLinkExtension,
      Placeholder.configure({ placeholder: "Type '/' for commands, '@' for people, or '#' for resources..." }),
    ],
    content: normalizeDocumentContent(value),
    immediatelyRender: false,
    editorProps: {
      attributes: {
        "aria-label": "Document content",
        class: "min-h-[28rem] outline-none",
      },
    },
    onUpdate: ({ editor: updated }) => onChange(updated.getJSON() as DocumentContent),
  });

  useEffect(() => {
    if (!editor || !loroSession) return;
    const plugins = createLoroEditorPlugins(loroSession);
    for (const plugin of plugins) editor.registerPlugin(plugin);
    return () => {
      for (const plugin of plugins) {
        if (plugin.spec.key) editor.unregisterPlugin(plugin.spec.key);
      }
    };
  }, [editor, loroSession]);

  useImperativeHandle(ref, () => ({
    insertAttachment: (attachment) => {
      if (!editor) return;
      editor.chain().focus().insertContent({
        type: "image",
        attrs: {
          src: attachment.src,
          alt: attachment.alt,
          attachmentId: attachment.attachmentId,
        },
      }).run();
    },
  }), [editor]);

  useEffect(() => {
    const nextValue = normalizeDocumentContent(value);
    if (!editor || JSON.stringify(editor.getJSON()) === JSON.stringify(nextValue)) return;
    editor.commands.setContent(nextValue, { emitUpdate: false });
  }, [editor, value]);

  const handleNodeChange = useCallback(({ pos }: { pos: number }) => {
    setActiveNodePos(pos);
  }, []);
  if (!editor) return null;

  const insertBlockAfter = (item: SlashCommandItem) => {
    if (item.id === "attachment") {
      onRequestAttachmentRef.current?.();
      setBlockMenuOpen(false);
      return;
    }
    if (activeNodePos === null) return;
    const node = editor.state.doc.nodeAt(activeNodePos);
    if (!node) return;
    const insertPos = activeNodePos + node.nodeSize;
    const content = item.id === "paragraph"
      ? { type: "paragraph" }
      : item.id === "heading-1"
        ? { type: "heading", attrs: { level: 1 } }
        : item.id === "heading-2"
          ? { type: "heading", attrs: { level: 2 } }
          : item.id === "heading-3"
            ? { type: "heading", attrs: { level: 3 } }
          : item.id === "bullet-list"
            ? { type: "bulletList", content: [{ type: "listItem", content: [{ type: "paragraph" }] }] }
            : item.id === "numbered-list"
              ? { type: "orderedList", content: [{ type: "listItem", content: [{ type: "paragraph" }] }] }
              : item.id === "task-list"
                ? { type: "taskList", content: [{ type: "taskItem", attrs: { checked: false }, content: [{ type: "paragraph" }] }] }
              : item.id === "quote"
                ? { type: "blockquote", content: [{ type: "paragraph" }] }
              : item.id === "callout"
                ? { type: "callout", attrs: { kind: "info" }, content: [{ type: "paragraph" }] }
                : item.id === "code-block"
                  ? { type: "codeBlock" }
                  : { type: "horizontalRule" };
    editor.chain().focus().insertContentAt(insertPos, content).setTextSelection(insertPos + 1).run();
    setBlockMenuOpen(false);
  };

  return (
    <div className="grid gap-3">
      <BubbleMenu editor={editor} className="flex items-center gap-0.5 rounded-lg border border-border bg-popover p-1 shadow-xl">
        <ToolbarButton label="Bold" active={editor.isActive("bold")} onClick={() => editor.chain().focus().toggleBold().run()}><Bold /></ToolbarButton>
        <ToolbarButton label="Italic" active={editor.isActive("italic")} onClick={() => editor.chain().focus().toggleItalic().run()}><Italic /></ToolbarButton>
        <ToolbarButton label="Strikethrough" active={editor.isActive("strike")} onClick={() => editor.chain().focus().toggleStrike().run()}><Strikethrough /></ToolbarButton>
        <ToolbarButton label="Inline code" active={editor.isActive("code")} onClick={() => editor.chain().focus().toggleCode().run()}><Code2 /></ToolbarButton>
        <span className="mx-0.5 h-5 w-px bg-border" />
        <ToolbarButton label="Bullet list" active={editor.isActive("bulletList")} onClick={() => editor.chain().focus().toggleBulletList().run()}><List /></ToolbarButton>
        <ToolbarButton label="Numbered list" active={editor.isActive("orderedList")} onClick={() => editor.chain().focus().toggleOrderedList().run()}><ListOrdered /></ToolbarButton>
        <ToolbarButton label="Task list" active={editor.isActive("taskList")} onClick={() => editor.chain().focus().toggleTaskList().run()}><ListTodo /></ToolbarButton>
        <ToolbarButton label="Quote" active={editor.isActive("blockquote")} onClick={() => editor.chain().focus().toggleBlockquote().run()}><Quote /></ToolbarButton>
        {onRequestAttachment ? <ToolbarButton label="Attachment" onClick={() => onRequestAttachmentRef.current?.()}><Paperclip /></ToolbarButton> : null}
        <ToolbarButton label="Link" active={editor.isActive("link")} onClick={() => {
          const href = window.prompt("Link URL");
          if (href) editor.chain().focus().setLink({ href }).run();
        }}><LinkIcon /></ToolbarButton>
      </BubbleMenu>
      <EditorContent
        editor={editor}
        className="typeset min-h-[28rem] max-w-none text-sm leading-7 [&_.ProseMirror]:min-h-[28rem] [&_.ProseMirror]:outline-none [&_.ProseMirror_p.is-editor-empty:first-child::before]:text-muted-foreground"
      />
      <DragHandle editor={editor} nested onNodeChange={handleNodeChange} className="z-10 flex h-6 items-center gap-0.5 rounded-md pr-2 text-muted-foreground/50 transition-colors hover:text-foreground active:cursor-grabbing">
        <Popover open={blockMenuOpen} onOpenChange={setBlockMenuOpen}>
          <PopoverTrigger
            render={<button type="button" aria-label="Add block" className="grid size-6 place-items-center rounded-md hover:bg-muted" />}
            onPointerDown={(event) => event.stopPropagation()}
          >
            <Plus className="size-4" />
          </PopoverTrigger>
          <PopoverContent side="right" align="start" className="w-80 p-1">
            <SlashCommandList items={slashCommands} command={insertBlockAfter} />
          </PopoverContent>
        </Popover>
        <span className="grid size-6 cursor-grab place-items-center rounded-md hover:bg-muted" aria-hidden="true">
          <GripVertical className="size-4" />
        </span>
      </DragHandle>
    </div>
  );
});

DocumentEditor.displayName = "DocumentEditor";
