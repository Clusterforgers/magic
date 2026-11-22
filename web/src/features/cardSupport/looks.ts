/** The look of each support state on the Card support page: its word, icon, bar segment and mark, from the support tokens. */
import {
  BanIcon,
  BookDashedIcon,
  BookOpenCheckIcon,
  ClockIcon,
  HandIcon,
  ShieldCheckIcon,
  ShieldHalfIcon,
} from 'lucide-react'
import type { LucideIcon } from 'lucide-react'
import { BY_HAND } from '@/lib/byHand'
import type { SupportStates } from '@/protocol/generated'

/** A state of the snapshot's breakdown. */
export type StateKey = keyof SupportStates

/** Word, meaning, icon, segment class and mark class of a state. */
export interface StateLook {
  word: string
  meaning: string
  Icon: LucideIcon
  segment: string
  mark: string
  automatic: boolean
}

// Whole class names, so Tailwind can see them.
export const LOOKS: Readonly<Record<StateKey, StateLook>> = {
  verified: {
    word: 'Verified',
    meaning: 'The checker proved the rules match the card.',
    Icon: ShieldCheckIcon,
    segment: 'bg-support-verified',
    mark: 'border-support-verified bg-support-verified/12 text-support-verified',
    automatic: true,
  },
  read: {
    word: 'Read',
    meaning: 'A player read the rules and approved them, or a trusted Forge translation.',
    Icon: BookOpenCheckIcon,
    segment: 'bg-support-read',
    mark: 'border-support-read bg-support-read/12 text-support-read',
    automatic: true,
  },
  unverified: {
    word: 'Unverified',
    meaning: 'Whole rules that play while they wait for a player to read them.',
    Icon: BookDashedIcon,
    segment: 'bg-support-unverified',
    mark: 'border-support-unverified bg-support-unverified/12 text-support-unverified',
    automatic: true,
  },
  waiting: {
    word: 'Waiting',
    meaning: `Partial rules wait for a read; ${BY_HAND} until then.`,
    Icon: ClockIcon,
    segment: 'bg-support-forge',
    mark: 'border-support-forge bg-support-forge/12 text-support-forge',
    automatic: false,
  },
  partial: {
    word: 'Partial',
    meaning: `Some clauses are ${BY_HAND}.`,
    Icon: ShieldHalfIcon,
    segment: 'bg-support-partial',
    mark: 'border-support-partial bg-support-partial/12 text-support-partial',
    automatic: false,
  },
  manual: {
    word: 'Manual',
    meaning: `The whole card is ${BY_HAND}.`,
    Icon: HandIcon,
    segment: 'bg-support-manual',
    mark: 'border-support-manual bg-support-manual/12 text-support-manual',
    automatic: false,
  },
  other: {
    word: 'Other',
    meaning: 'Stale or unplayable rules.',
    Icon: BanIcon,
    segment: 'bg-support-unplayable',
    mark: 'border-support-unplayable bg-support-unplayable/12 text-support-unplayable',
    automatic: false,
  },
}

/** The states in the order of the bar: automatic first. */
export const STATE_ORDER: readonly StateKey[] = [
  'verified',
  'read',
  'unverified',
  'waiting',
  'partial',
  'manual',
  'other',
]
