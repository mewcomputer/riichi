import { getProductIcon } from "@/lib/product-icons";

export function isTeamIcon(value: string | null | undefined) {
  return Boolean(value?.startsWith("tabler:") || value?.startsWith("lucide:"));
}

export function teamMarkLabel(value: string | null | undefined) {
  return isTeamIcon(value) ? "✦" : value || "◈";
}

export function TeamMark({ value, className = "" }: { value?: string | null; className?: string }) {
  if (!value) return <span className={className}>◈</span>;
  if (!isTeamIcon(value)) return <span className={className}>{value}</span>;
  const prefixLength = value.startsWith("tabler:") ? "tabler:".length : "lucide:".length;
  const rawName = value.slice(prefixLength);
  const name = rawName.includes("-")
    ? rawName.split("-").map((part) => part ? part[0].toUpperCase() + part.slice(1) : part).join("")
    : rawName[0]?.toUpperCase() + rawName.slice(1);
  const Icon = getProductIcon(name);
  return Icon ? <Icon className={className} aria-hidden="true" /> : <span className={className}>✦</span>;
}
