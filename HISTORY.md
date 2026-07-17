# HISTORY

## 2026-07-13 — Rich text editors for issue detail

Converted the issue detail page's title and body fields from plain HTML inputs
to Tiptap rich-text editors.

### Changes

- **New component: `apps/web/src/components/issues/rich-text-issue-editor.tsx`**
  - `RichTextTitleEditor` — plain-text Tiptap editor for the issue title.
    Enter is suppressed to keep it single-line. Serializes via `getText()`
    so the `title: string` API contract is unchanged. Ghost text: "Issue title".
  - `RichTextBodyEditor` — rich-text Tiptap editor for the issue body.
    Uses `StarterKit` with a floating `BubbleMenu` toolbar (bold, italic,
    bullet list) that appears on text selection. Serializes via `getHTML()`
    so the body becomes rich HTML. Ghost text: "Add context...".
  - Both editors are controlled (`value` / `onChange` props) and sync
    external value changes (e.g. navigating to a different issue) via
    `useEffect` + `setContent`.
  - Both accept an optional `className` prop for custom styling.

- **Installed `@tiptap/extension-placeholder`** — provides ghost text that
  shows when the editor is empty and disappears on input.

- **Modified `apps/web/src/routes/issue-detail.tsx`**
  - Replaced the `<Input>` (title) and `<Textarea>` (body) with
    `<RichTextTitleEditor>` and `<RichTextBodyEditor>`.
  - Removed the now-unused `Textarea` import.
  - Added import for the new editors.

- **Modified `apps/web/src/index.css`**
  - Added `.ProseMirror p.is-editor-empty:first-child::before` rule to
    style the placeholder text using the theme's `--muted-foreground`
    color so it adapts to all themes.

### Notes

- Existing plain-text bodies load fine — Tiptap wraps them in a paragraph.
- The backend builds agent context from the body as plain text
  (`format!("# {}\n\n{}", issue.title, issue.body)` in
  `crates/persistence/src/context.rs`). Now that the body is HTML, the
  agent context will contain HTML tags. A follow-up to strip tags there
  may be desirable.
