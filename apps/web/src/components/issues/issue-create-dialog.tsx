import { useEffect, useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { RichTextBodyEditor, RichTextTitleEditor } from "./rich-text-issue-editor";

export function IssueCreateDialog({
  open,
  onOpenChange,
  onSubmit,
  parentIssueId,
  submitting = false,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (input: { title: string; body: string; parent_issue_id?: string }) => void;
  parentIssueId?: string;
  submitting?: boolean;
}) {
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");

  useEffect(() => {
    if (!open) {
      setTitle("");
      setBody("");
    }
  }, [open]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{parentIssueId ? "New sub-issue" : "New issue"}</DialogTitle>
          <DialogDescription>{parentIssueId ? "Break this issue into a smaller piece of work." : "Create a piece of work for the project queue."}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-4">
          <div className="grid gap-2">
            <label className="text-sm font-medium" htmlFor="issue-title">Title</label>
            <div id="issue-title" className="rounded-md border border-border/60 px-3 py-2 focus-within:border-ring/60">
              <RichTextTitleEditor
                value={title}
                onChange={setTitle}
                className="text-base font-medium [&_.ProseMirror]:min-h-6 [&_.ProseMirror]:outline-none"
              />
            </div>
          </div>
          <div className="grid gap-2">
            <label className="text-sm font-medium" htmlFor="issue-body">Description</label>
            <div id="issue-body" className="rounded-md border border-border/60 px-3 py-2 focus-within:border-ring/60">
              <RichTextBodyEditor
                value={body}
                onChange={setBody}
                className="typeset min-h-28 max-w-none text-sm leading-6 [&_.ProseMirror]:min-h-24 [&_.ProseMirror]:outline-none"
              />
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button
            disabled={submitting || !title.trim()}
            onClick={() => onSubmit({ title: title.trim(), body, ...(parentIssueId ? { parent_issue_id: parentIssueId } : {}) })}
          >
            {submitting ? "Creating..." : "Create issue"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
