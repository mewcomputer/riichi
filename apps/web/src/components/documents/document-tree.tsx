import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ChevronRight, FileText, Plus } from "lucide-react";

import type { DocumentRecord } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { Link } from "@tanstack/react-router";

const MAX_DOCUMENT_TREE_DEPTH = 8;

type DocumentTreeProps = {
  organizationSlug: string;
  documents: DocumentRecord[];
  listChildren: (parentDocumentId?: string) => Promise<DocumentRecord[]>;
  onCreate?: (parentDocumentId?: string) => void;
};

export function DocumentTree({ organizationSlug, documents, listChildren, onCreate }: DocumentTreeProps) {
  if (documents.length === 0) {
    return <div className="rounded-md border border-dashed border-border/70 px-4 py-5 text-xs text-muted-foreground">No documents yet.</div>;
  }

  return (
    <div className="grid gap-0.5">
      {documents.map((document) => (
        <DocumentTreeNode
          key={document.id}
          document={document}
          depth={0}
          organizationSlug={organizationSlug}
          listChildren={listChildren}
          onCreate={onCreate}
        />
      ))}
    </div>
  );
}

function DocumentTreeNode({
  document,
  depth,
  organizationSlug,
  listChildren,
  onCreate,
}: Omit<DocumentTreeProps, "documents"> & { document: DocumentRecord; depth: number }) {
  const [expanded, setExpanded] = useState(false);
  const canExpand = depth < MAX_DOCUMENT_TREE_DEPTH;
  const childrenQuery = useQuery({
    queryKey: ["document-children", document.id],
    queryFn: () => listChildren(document.id),
    enabled: expanded && canExpand,
  });

  return (
    <div>
      <div className="group flex min-w-0 items-center gap-1 rounded-md pr-1 hover:bg-muted/40">
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className={cn("size-7 shrink-0 text-muted-foreground", !canExpand && "invisible")}
          aria-label={expanded ? `Collapse ${document.title}` : `Expand ${document.title}`}
          aria-expanded={expanded}
          onClick={() => setExpanded((current) => !current)}
        >
          <ChevronRight className={cn("size-3.5 transition-transform", expanded && "rotate-90")} />
        </Button>
        <Link
          to="/$organizationSlug/documents/$documentId"
          params={{ organizationSlug, documentId: document.id }}
          className="flex min-w-0 flex-1 items-center gap-2 py-1.5 text-sm"
          style={{ paddingLeft: `${depth * 16}px` }}
        >
          <FileText className="size-3.5 shrink-0 text-muted-foreground" />
          <span className="truncate">{document.title}</span>
        </Link>
        {onCreate ? (
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="size-7 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
            aria-label={`New page inside ${document.title}`}
            onClick={() => onCreate(document.id)}
          >
            <Plus className="size-3.5" />
          </Button>
        ) : null}
      </div>
      {expanded && canExpand && childrenQuery.data?.length ? (
        <div>
          {childrenQuery.data.map((child) => (
            <DocumentTreeNode
              key={child.id}
              document={child}
              depth={depth + 1}
              organizationSlug={organizationSlug}
              listChildren={listChildren}
              onCreate={onCreate}
            />
          ))}
        </div>
      ) : null}
      {expanded && canExpand && childrenQuery.isFetched && !childrenQuery.data?.length ? (
        <div className="py-1 pl-12 text-[11px] text-muted-foreground">No child pages.</div>
      ) : null}
    </div>
  );
}
