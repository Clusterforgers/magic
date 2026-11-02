/** The words of a seat's companion slot: where the card is and what the viewer may do about it. */
import type { CompanionView } from '@/protocol/generated'

/** What the slot says: the short label, the sentence for a screen reader and the card's name when the viewer may see it. */
export interface CompanionFacts {
  label: string
  words: string
  name: string | undefined
}

const WHERE = {
  Hidden: 'not revealed yet',
  Revealed: 'outside the game',
  Brought: 'in hand',
} as const

/** The slot of a companion for the seat's `owner` name: its label, and its place in words. */
export function companionFacts(companion: CompanionView, owner: string): CompanionFacts {
  const where = WHERE[companion.stage]
  const name = companion.name
  const label = name ?? 'Companion'
  const subject = name === undefined ? 'companion' : `companion ${name}`
  return { label, name, words: `${owner}'s ${subject}: ${where}` }
}

/** The text of the button that brings the companion into the hand. */
export function bringLabel(name: string | undefined): string {
  return name === undefined ? 'Bring companion ({3})' : `Bring ${name} to hand ({3})`
}
