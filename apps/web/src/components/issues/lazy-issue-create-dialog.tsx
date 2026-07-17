import { lazy, Suspense, type ComponentProps } from "react";

const IssueCreateDialog = lazy(() =>
  import("./issue-create-dialog").then(({ IssueCreateDialog: component }) => ({ default: component })),
);

export function LazyIssueCreateDialog(props: ComponentProps<typeof IssueCreateDialog>) {
  if (!props.open) return null;
  return (
    <Suspense fallback={null}>
      <IssueCreateDialog {...props} />
    </Suspense>
  );
}
