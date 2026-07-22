import { RichTextComment, RichTextCommentEditor } from "./rich-text-comment-editor";
import type { ActivityRecord, CommentRecord } from "@/lib/api";

type TimelineEntry =
  | { type: "comment"; comment: CommentRecord }
  | { type: "activity"; activities: ActivityRecord[]; topic: string };

function changes(activity: ActivityRecord) {
  return Array.isArray(activity.metadata.changes)
    ? activity.metadata.changes.filter((change): change is Record<string, unknown> => typeof change === "object" && change !== null)
    : [];
}

function topic(activity: ActivityRecord) {
  const fields = changes(activity)
    .map((change) => typeof change.field === "string" ? change.field : null)
    .filter((field): field is string => Boolean(field));
  return fields.length > 0 ? fields.sort().join(", ") : "issue";
}

function value(value: unknown) {
  if (typeof value === "string") return value.replaceAll("_", " ");
  if (typeof value === "boolean") return value ? "yes" : "no";
  if (value === null || value === undefined) return "none";
  return JSON.stringify(value);
}

function diff(activity: ActivityRecord) {
  return changes(activity).map((change) => {
    const field = typeof change.field === "string" ? change.field : "issue";
    if (change.changed === true) return field;
    if ("from" in change && "to" in change) return `${field}: ${value(change.from)} → ${value(change.to)}`;
    return field;
  });
}

const undoableFields = new Set(["status", "importance", "agent eligibility", "specification", "rank", "labels"]);

function undoValue(activity: ActivityRecord) {
  const change = changes(activity).find((candidate) => undoableFields.has(String(candidate.field)) && "from" in candidate && "to" in candidate);
  if (!change) return null;
  return { field: String(change.field), value: change.from };
}

function statusSummary(activities: ActivityRecord[]) {
  const transitions = activities.flatMap((activity) => changes(activity).filter((change) => change.field === "status" && "from" in change && "to" in change));
  if (transitions.length === 0) return null;
  return `updated status from ${value(transitions[0].from)} to ${value(transitions[transitions.length - 1].to)}`;
}

const eventLabels: Record<string, string> = {
  create_issue: "created this issue",
  update_issue: "updated this issue",
  create_comment: "commented",
  claim: "claimed an agent lease",
  renew_lease: "renewed an agent lease",
  release_lease: "released an agent lease",
  report_batch: "reported agent progress",
  takeover_issue: "took over agent work",
  create_approval_request: "requested approval",
  approve_approval_request: "approved a proposed change",
  reject_approval_request: "rejected a proposed change",
  supersede_approval_request: "superseded an approval",
  document_edit: "edited the description",
  create_hold: "placed the issue on hold",
  release_hold: "released an issue hold",
  create_issue_edge: "changed an issue relationship",
  remove_issue_edge: "removed an issue relationship",
};

function eventLabel(kind: string) {
  return eventLabels[kind] ?? kind.replaceAll("_", " ");
}

function compactTimeline(comments: CommentRecord[], activities: ActivityRecord[]) {
  const entries: TimelineEntry[] = [];
  const groups = new Map<string, ActivityRecord[]>();
  const events = [
    ...comments.map((comment) => ({ type: "comment" as const, createdAt: comment.created_at, comment })),
    ...activities
      .filter((activity) => activity.kind !== "comment" && activity.kind !== "create_comment")
      .map((activity) => ({ type: "activity" as const, createdAt: activity.created_at, activity })),
  ].sort((left, right) => left.createdAt.localeCompare(right.createdAt));

  for (const event of events) {
    if (event.type === "comment") {
      entries.push({ type: "comment", comment: event.comment });
      continue;
    }
    const activityTopic = topic(event.activity);
    const key = `${event.activity.actor_id}:${activityTopic}`;
    const group = groups.get(key);
    if (group) group.push(event.activity);
    else {
      const created = [event.activity];
      groups.set(key, created);
      entries.push({ type: "activity", activities: created, topic: activityTopic });
    }
  }
  return entries.sort((left, right) => {
    const leftCreatedAt = left.type === "comment"
      ? left.comment.created_at
      : left.activities[left.activities.length - 1].created_at;
    const rightCreatedAt = right.type === "comment"
      ? right.comment.created_at
      : right.activities[right.activities.length - 1].created_at;
    return leftCreatedAt.localeCompare(rightCreatedAt);
  });
}

export function IssueActivityTimeline({
  comments,
  activities,
  canComment,
  submitting = false,
  error,
  onSubmit,
  onUndo,
  undoing = false,
}: {
  comments: CommentRecord[];
  activities: ActivityRecord[];
  canComment: boolean;
  submitting?: boolean;
  error?: Error;
  onSubmit: (content: Record<string, unknown>) => void;
  onUndo?: (activity: ActivityRecord, field: string, value: unknown) => void;
  undoing?: boolean;
}) {
  const entries = compactTimeline(comments, activities);
  return (
    <section className="grid gap-3 border-t border-border/60 pt-5">
      <div>
        <h2 className="text-sm font-medium">Activity</h2>
        <p className="text-xs text-muted-foreground">Comments and issue history.</p>
      </div>
      {entries.length > 0 ? <div className="grid gap-3">
        {entries.map((entry) => {
          if (entry.type === "comment") {
            return <article key={entry.comment.id} className="rounded-md border border-border/60 bg-card/20 p-3 text-sm">
              <div className="mb-2 flex items-center gap-2 text-[10px] text-muted-foreground"><span className="font-mono">{entry.comment.author_id.slice(0, 8)}</span><span>·</span><time dateTime={entry.comment.created_at}>{new Date(entry.comment.created_at).toLocaleString()}</time></div>
              <RichTextComment content={entry.comment.content} fallback={entry.comment.body} />
            </article>;
          }
          const first = entry.activities[0];
          const summary = statusSummary(entry.activities);
          return <div key={first.id} className="flex items-start gap-3 text-xs">
            <span className="mt-1 size-1.5 shrink-0 rounded-full bg-muted-foreground/60" />
            <details className="min-w-0">
              <summary className="cursor-pointer list-none">
                <span className="font-medium">{first.actor_id.slice(0, 8)} {summary ?? eventLabel(first.kind)}{entry.activities.length > 1 ? ` (${entry.activities.length})` : ""}</span>
                {summary ? null : <span className="ml-2 text-muted-foreground">{entry.topic}</span>}
              </summary>
              <div className="mt-2 grid gap-2 border-l border-border/60 pl-3">
                {entry.activities.map((activity) => {
                  const inverse = undoValue(activity);
                  return <div key={activity.id} className="flex items-center gap-2 text-muted-foreground"><span className="min-w-0 flex-1">{diff(activity).join(" · ") || eventLabel(activity.kind)}<span className="ml-2 text-[10px]">· {new Date(activity.created_at).toLocaleString()}</span></span>{inverse && onUndo ? <button type="button" className="shrink-0 text-[10px] text-foreground underline underline-offset-2 disabled:opacity-50" onClick={() => onUndo(activity, inverse.field, inverse.value)} disabled={undoing}>Undo</button> : null}</div>;
                })}
              </div>
            </details>
          </div>;
        })}
      </div> : <p className="text-xs text-muted-foreground">No activity yet.</p>}
      {canComment ? <RichTextCommentEditor submitting={submitting} onSubmit={onSubmit} /> : <p className="text-xs text-muted-foreground">You have read-only access to this project.</p>}
      {error ? <span className="text-xs text-destructive">{error.message}</span> : null}
    </section>
  );
}
