/** The icon of each panel row, so a row reads as a command before its words do. */
import {
  ArrowDownToLineIcon,
  ArrowRightLeftIcon,
  BanIcon,
  ChevronsUpIcon,
  DicesIcon,
  DropletsIcon,
  EyeIcon,
  HandIcon,
  HashIcon,
  HeartIcon,
  Link2OffIcon,
  LinkIcon,
  RotateCwIcon,
  ScanEyeIcon,
  ShuffleIcon,
  SkullIcon,
  SparklesIcon,
  StarIcon,
  SwordIcon,
  Trash2Icon,
  ZapIcon,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import type { CardCommandId } from './cardCommands'
import type { CommandId } from './commands'

/** The icon of each command of the panel. */
export const COMMAND_ICONS: Record<CommandId, LucideIcon> = {
  life: HeartIcon,
  poison: SkullIcon,
  energy: ZapIcon,
  experience: StarIcon,
  draw: HandIcon,
  look: EyeIcon,
  mill: Trash2Icon,
  exileTop: BanIcon,
  bottomTop: ArrowDownToLineIcon,
  shuffle: ShuffleIcon,
  revealTop: ScanEyeIcon,
  token: SparklesIcon,
  mana: DropletsIcon,
  roll: DicesIcon,
}

/** The icon of each command of the picked card. */
export const CARD_COMMAND_ICONS: Record<CardCommandId, LucideIcon> = {
  move: ArrowRightLeftIcon,
  discard: Trash2Icon,
  tap: RotateCwIcon,
  counters: HashIcon,
  attach: LinkIcon,
  unattach: Link2OffIcon,
  damage: SwordIcon,
  pump: ChevronsUpIcon,
}
