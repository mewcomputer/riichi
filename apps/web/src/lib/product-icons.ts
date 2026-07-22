import { icons } from "@tabler/icons-react";
import type { ComponentType, SVGProps } from "react";

export type ProductIcon = ComponentType<SVGProps<SVGSVGElement>>;

export const {
  IconArrowLeft: ArrowLeft,
  IconArrowUpRight: ArrowUpRight,
  IconArchive: Archive,
  IconAdjustmentsHorizontal: SlidersHorizontal,
  IconAlertCircle: CircleAlert,
  IconAlertTriangle: AlertTriangle,
  IconBell: Bell,
  IconBold: Bold,
  IconCheck: Check,
  IconChevronDown: ChevronDown,
  IconChevronRight: ChevronRight,
  IconCircle: Circle,
  IconCircleCheck: CircleCheck,
  IconCircleDot: CircleDot,
  IconCircleX: CircleX,
  IconCommand: Command,
  IconExternalLink: ExternalLink,
  IconFileText: FileText,
  IconFilter: Filter,
  IconFlag: Flag,
  IconFolder: FolderKanban,
  IconItalic: Italic,
  IconKeyboard: Keyboard,
  IconStack3: Layers3,
  IconLink: Link2,
  IconList: List,
  IconListCheck: ListTodo,
  IconListNumbers: ListOrdered,
  IconLoader2: LoaderCircle,
  IconMenu2: MoreHorizontal,
  IconPin: Pin,
  IconPinnedOff: PinOff,
  IconPlus: Plus,
  IconRefresh: RefreshCw,
  IconRobot: Bot,
  IconSearch: Search,
  IconSend: Send,
  IconSettings: Settings2,
  IconShield: ShieldAlert,
  IconTypography: Type,
  IconUser: UserRound,
  IconUsers: UsersRound,
  IconX: X,
} = icons;

export const Building2 = icons.IconBuilding;
export const Folder = FolderKanban;
export const Code2 = icons.IconCode;
export const GripVertical = icons.IconGripVertical;
export const Link = icons.IconLink;
export const Minus = icons.IconMinus;
export const Paperclip = icons.IconPaperclip;
export const Quote = icons.IconQuote;
export const Strikethrough = icons.IconStrikethrough;
export const Users = icons.IconUsers;
export const ListFilter = icons.IconFilter;
export const ClipboardCheck = icons.IconClipboardCheck;

export const productIconNames = Object.keys(icons)
  .filter((name) => name.startsWith("Icon"))
  .map((name) => name.slice(4))
  .filter((name) => !name.endsWith("Filled") && !name.endsWith("Off"))
  .sort();

export function getProductIcon(name: string): ProductIcon | undefined {
  return icons[`Icon${name}` as keyof typeof icons] as ProductIcon | undefined;
}
