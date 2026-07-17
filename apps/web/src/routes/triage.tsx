import { useParams } from "@tanstack/react-router";
import { QueuePage } from "./queue";

export function TriagePage() {
  const { organizationSlug } = useParams({ from: "/$organizationSlug/triage" });
  return <QueuePage initialFilter="held" organizationSlug={organizationSlug} />;
}
