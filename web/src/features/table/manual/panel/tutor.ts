/** Pure: the cards of a library search by hand in the order the grid shows them, and what taking one sends. */
import type { ObjectId } from '@/protocol/generated'
import type { Preview } from './commands'

/** One card of the search grid. */
export interface Found {
  id: ObjectId
  name: string
}

/** The searched cards by name, so the library's order is not on show; cards of one name keep their order. */
export function foundCards(
  searched: readonly ObjectId[],
  nameOf: (card: ObjectId) => string | undefined,
): Found[] {
  return searched
    .map((id) => ({ id, name: nameOf(id) ?? 'A card' }))
    .sort((a, b) => a.name.localeCompare(b.name))
}

/** What taking `card` from the library changes, or why nothing is chosen yet. */
export function tutorPreview(card: Found | undefined, library: number): Preview {
  if (card === undefined) return { problem: 'Choose a card to put into your hand.' }
  return {
    preview: `${card.name}: library → hand; the ${library - 1} other cards are shuffled and the other seats learn only that a card went to your hand`,
    apply: `Put ${card.name} into your hand`,
    action: { Tutor: { object: card.id } },
  }
}

/** Ending the search without a card: the library is shuffled. */
export const FIND_NOTHING = { label: 'Shuffle without a card', action: { Shuffle: {} } } as const
