import { TeamMark } from "@/components/team/team-mark";
import { Folder } from "@/lib/product-icons";

export function ProjectMark({ value, className = "" }: { value?: string | null; className?: string }) {
  return value ? <TeamMark value={value} className={className} /> : <Folder className={className} aria-hidden="true" />;
}
